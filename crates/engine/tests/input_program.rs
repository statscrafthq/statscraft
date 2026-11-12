//! INPUT PROGRAM, END CASE, END FILE, LEAVE, REREAD, REPEATING DATA and FILE TYPE on made-up
//! records (a parcel round, room sensors, meter readings, a parts store, truck loads, invoices,
//! a sports club, a running club and farm visits), inline or in files. The expected values are
//! read off the records by hand.

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
    let (s, result) = ran("INPUT PROGRAM.\n\
         DATA LIST /#S1 TO #S4 (4(F1,1X)).\n\
         VECTOR SCORE=#S1 TO #S4.\n\
         LOOP #K=1 TO 4.\n\
         - COMPUTE PTS=SCORE(#K).\n\
         - END CASE.\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         4 0 7 2\n\
         9 3 3 8\n\
         END DATA.\n\
         FORMAT PTS(F1.0).\n\
         PRINT / PTS.\n\
         EXECUTE.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["PTS"]);
    assert_eq!(whole(data, "PTS"), [4.0, 0.0, 7.0, 2.0, 9.0, 3.0, 3.0, 8.0]);
    assert_eq!(printed(&result), ["4", "0", "7", "2", "9", "3", "3", "8"]);
}

#[test]
fn leave_gives_the_route_number_to_each_stop() {
    // a route number, then up to six pairs of a stop and its parcel count
    let (s, result) = ran("INPUT PROGRAM.\n\
         DATA LIST /ROUTE 1-3 #P1 TO #P12 (1X,6(F2.0,1X,F1.0,1X)).\n\
         LEAVE ROUTE.\n\
         VECTOR PAIR=#P1 TO #P12.\n\
         LOOP #J=1 TO 11 BY 2 IF NOT SYSMIS(PAIR(#J)).\n\
         - COMPUTE STOP=PAIR(#J).\n\
         - COMPUTE PARCELS=PAIR(#J+1).\n\
         - END CASE.\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         301 12 3 07 1 15 5\n\
         302 07 2 20 4\n\
         303 15 1 12 2 09 6 20 1\n\
         304 09 3\n\
         305 20 2 07 1 12 4 03 2 15 1 09 1\n\
         END DATA.\n\
         SORT CASES STOP.\n\
         DO IF $CASENUM EQ 1.\n\
         - PRINT EJECT /'Route Stop Parcels'.\n\
         - PRINT SPACE.\n\
         END IF.\n\
         FORMATS STOP (F2)/ PARCELS (F1).\n\
         PRINT /' ' ROUTE ' ' STOP '  ' PARCELS.\n\
         EXECUTE.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["ROUTE", "STOP", "PARCELS"]);
    let rows: Vec<(f64, f64, f64)> = (0..data.case_count())
        .map(|i| {
            let get = |n: &str| whole(data, n)[i];
            (get("ROUTE"), get("STOP"), get("PARCELS"))
        })
        .collect();
    // sorted by stop, the routes of one stop in the order they were read
    let expected = [
        (305, 3, 2),
        (301, 7, 1),
        (302, 7, 2),
        (305, 7, 1),
        (303, 9, 6),
        (304, 9, 3),
        (305, 9, 1),
        (301, 12, 3),
        (303, 12, 2),
        (305, 12, 4),
        (301, 15, 5),
        (303, 15, 1),
        (305, 15, 1),
        (302, 20, 4),
        (303, 20, 1),
        (305, 20, 2),
    ];
    let expected: Vec<(f64, f64, f64)> = expected
        .iter()
        .map(|&(a, b, c)| (f64::from(a), f64::from(b), f64::from(c)))
        .collect();
    assert_eq!(rows, expected);
    let lines = printed(&result);
    assert_eq!(lines[0], "Route Stop Parcels");
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], " 305   3   2");
    assert_eq!(lines.len(), 18);
}

#[test]
fn an_input_program_without_data_makes_cases_from_a_loop() {
    let (s, result) = ran("SET FORMAT=F8.2.\n\
         INPUT PROGRAM.\n\
         LOOP N=1 TO 500.\n\
         + COMPUTE DRAW=UNIFORM(10).\n\
         + END CASE.\n\
         END LOOP.\n\
         END FILE.\n\
         END INPUT PROGRAM.\n\
         FREQUENCIES VARIABLES=DRAW /FORMAT=NOTABLE /HISTOGRAM\n\
          /PERCENTILES=5 25 50 75 95 /STATISTICS=MEAN MEDIAN.");
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 500);
    let index = whole(data, "N");
    assert_eq!(index, (1..=500).map(f64::from).collect::<Vec<_>>());
    let draws = whole(data, "DRAW");
    assert!(draws.iter().all(|d| (0.0..10.0).contains(d)));
    // a uniform variable on 0 to 10 has its median at 5
    let below = draws.iter().filter(|d| **d < 5.0).count();
    assert!((190..310).contains(&below), "{below}");
    assert!(!tables(&result).is_empty());
}

/// A header record per room (the count of readings and a two-letter code), then the readings.
const ROOMS: &str = "BEGIN DATA\n\
     2 NE\n\
     215\n\
     198\n\
     0 ZZ\n\
     1 SW\n\
     224\n\
     3 NW\n\
     187\n\
     203\n\
     211\n\
     END DATA.\n";

#[test]
fn end_case_gives_the_header_to_each_reading() {
    let (s, result) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /#N 1 BLDG LEVEL 3-4(A).\n\
         LEAVE BLDG LEVEL.\n\
         LOOP #I=1 TO #N.\n\
         DATA LIST /TEMP 1-3(1).\n\
         PRINT /#N BLDG LEVEL TEMP.\n\
         END CASE.\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         {ROOMS}LIST."
    ));
    assert_eq!(
        printed(&result),
        [
            "2 N E 21.5",
            "2 N E 19.8",
            "1 S W 22.4",
            "3 N W 18.7",
            "3 N W 20.3",
            "3 N W 21.1"
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["BLDG", "LEVEL", "TEMP"]);
    assert_eq!(strings(data, "BLDG"), ["N", "N", "S", "N", "N", "N"]);
    assert_eq!(strings(data, "LEVEL"), ["E", "E", "W", "W", "W", "W"]);
    assert_eq!(whole(data, "TEMP"), [21.5, 19.8, 22.4, 18.7, 20.3, 21.1]);
}

#[test]
fn without_end_case_each_run_of_the_program_is_one_case() {
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /#N 1 BLDG LEVEL 3-4(A).\n\
         LOOP #I=1 TO #N.\n\
         DATA LIST /TEMP 1-3(1).\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         {ROOMS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "BLDG"), ["N", "Z", "S", "N"]);
    assert_eq!(strings(data, "LEVEL"), ["E", "Z", "W", "W"]);
    assert_eq!(
        numbers(data, "TEMP"),
        [Some(19.8), None, Some(22.4), Some(21.1)]
    );
}

/// Day, shift and the meter reading in tenths of a kWh.
const METER: &str = "01 1  4520\n02 2  4877\n03 1  5012\n04 2  5190\n05 1  5333\n";

#[test]
fn end_file_stops_before_the_case_that_runs_it() {
    let dir = folder("meter");
    std::fs::write(dir.join("meter.txt"), METER).unwrap();
    let (s, _) = ran(&format!(
        "CD '{}'.\n\
         INPUT PROGRAM.\n\
         DATA LIST FILE='meter.txt' /DAY 1-2 SHIFT 4 KWH 6-10(1).\n\
         DO IF (DAY GE 4).\n\
         END FILE.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         LIST.",
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "DAY"), [1.0, 2.0, 3.0]);
    assert_eq!(whole(data, "KWH"), [452.0, 487.7, 501.2]);

    // END CASE before END FILE keeps the case of day 4
    let (s, _) = ran(&format!(
        "CD '{}'.\n\
         INPUT PROGRAM.\n\
         DATA LIST FILE='meter.txt' /DAY 1-2 SHIFT 4 KWH 6-10(1).\n\
         DO IF (DAY GE 4).\n\
         END CASE.\n\
         END FILE.\n\
         ELSE.\n\
         END CASE.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         LIST.",
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "DAY"), [1.0, 2.0, 3.0, 4.0]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_data_list_without_variables_skips_records() {
    // three title lines come before the rainfall figures
    let (s, _) = ran("INPUT PROGRAM.\n\
         NUMERIC #DONE.\n\
         DO IF #DONE = 0.\n\
         LOOP #I = 1 TO 3.\n\
         DATA LIST NOTABLE /.\n\
         END LOOP.\n\
         COMPUTE #DONE = 1.\n\
         END IF.\n\
         DATA LIST NOTABLE /RAIN 1-2.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         Station log\n\
         units mm\n\
         =====\n\
         12\n\
         0\n\
         7\n\
         END DATA.\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["RAIN"]);
    assert_eq!(whole(data, "RAIN"), [12.0, 0.0, 7.0]);
}

/// Item names in columns 1-10 and sales in 16-20.
fn sales_file() -> String {
    [("Pens", 1250), ("Paper", 4700), ("Ink", 980)]
        .iter()
        .map(|(item, amount)| format!("{item:<15}{amount:>5}\n"))
        .collect()
}

#[test]
fn the_end_variable_lets_the_program_print_a_total() {
    let dir = folder("sales");
    std::fs::write(dir.join("sales.txt"), sales_file()).unwrap();
    let (s, result) = ran(&format!(
        "CD '{}'.\n\
         FILE HANDLE SALES /NAME='sales.txt'.\n\
         INPUT PROGRAM.\n\
         NUMERIC TOTAL (DOLLAR8.0).\n\
         LEAVE TOTAL.\n\
         DO IF $CASENUM EQ 1.\n\
         PRINT EJECT.\n\
         PRINT / 'Item         Sales'.\n\
         END IF.\n\
         DATA LIST FILE=SALES END=#LAST NOTABLE / ITEM 1-10(A) AMOUNT 16-20(F).\n\
         DO IF #LAST.\n\
         PRINT / 'ALL ITEMS ', TOTAL.\n\
         END FILE.\n\
         ELSE.\n\
         PRINT / ITEM, AMOUNT (A10,COMMA8).\n\
         COMPUTE TOTAL = TOTAL + AMOUNT.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         EXECUTE.",
        dir.display()
    ));
    assert_eq!(
        printed(&result),
        [
            "Item         Sales",
            "Pens         1,250",
            "Paper        4,700",
            "Ink            980",
            "ALL ITEMS   $6,930"
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["TOTAL", "ITEM", "AMOUNT"]);
    assert_eq!(whole(data, "TOTAL"), [1250.0, 5950.0, 6930.0]);
    std::fs::remove_dir_all(dir).unwrap();
}

/// The shops of three regions, each file with its own columns: the north file has the shop in
/// 1-12, the staff count in 14-15 and the town in 17-18; the south file the count in 20-21 and
/// the town in 23-24; the east file starts two columns to the right of the north one.
fn shop_files() -> [(&'static str, String); 3] {
    let north: String = [("Harbour", 12, "TA"), ("Mill Lane", 7, "TA")]
        .iter()
        .map(|(shop, staff, town)| format!("{shop:<12} {staff:>2} {town}\n"))
        .collect();
    let south = format!("{:<19}{:>2} {}\n", "Riverside", 9, "PA");
    let east: String = [("Old Square", 15, "NA"), ("Station", 4, "TA")]
        .iter()
        .map(|(shop, staff, town)| format!("  {shop:<12} {staff:>2} {town}\n"))
        .collect();
    [("north.txt", north), ("south.txt", south), ("east.txt", east)]
}

/// Reads the three files one after the other, each until its END variable turns 1.
const THREE_FILES: &str = "INPUT PROGRAM.\n\
     NUMERIC #DONE1 #DONE2 #DONE3.\n\
     DO IF NOT #DONE1.\n\
     DATA LIST FILE='north.txt' END=#DONE1 NOTABLE /1 SHOP 1-12(A) STAFF 14-15 TOWN 17-18(A).\n\
     DO IF NOT #DONE1.\n\
     END CASE.\n\
     END IF.\n\
     ELSE IF NOT #DONE2.\n\
     DATA LIST FILE='south.txt' END=#DONE2 NOTABLE / SHOP 1-12(A) STAFF 20-21 TOWN 23-24(A).\n\
     DO IF NOT #DONE2.\n\
     END CASE.\n\
     END IF.\n\
     ELSE IF NOT #DONE3.\n\
     DATA LIST FILE='east.txt' END=#DONE3 NOTABLE / SHOP 3-14(A) STAFF 16-17 TOWN 19-20 (A).\n\
     DO IF NOT #DONE3.\n\
     END CASE.\n\
     END IF.\n\
     ELSE.\n\
     END FILE.\n\
     END IF.\n\
     END INPUT PROGRAM.\n";

#[test]
fn end_variables_join_three_files() {
    let dir = folder("three");
    for (name, text) in shop_files() {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let (s, _) = ran(&format!(
        "CD '{}'.\n{THREE_FILES}LIST VARIABLES=SHOP STAFF TOWN.",
        dir.display()
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["SHOP", "STAFF", "TOWN"]);
    assert_eq!(
        strings(data, "SHOP"),
        ["Harbour", "Mill Lane", "Riverside", "Old Square", "Station"]
    );
    assert_eq!(whole(data, "STAFF"), [12.0, 7.0, 9.0, 15.0, 4.0]);
    assert_eq!(strings(data, "TOWN"), ["TA", "TA", "PA", "NA", "TA"]);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A parts store: the shelf in 1-2 and the kind in 10-14. A GEAR record has its cost in 3-6
/// and its stock in 7-9, a BELT record the cost in 15-18 and the stock in 19-21; other kinds
/// are not read.
const PARTS: &str = "BEGIN DATA\n\
     04 875012GEAR      SPROCKET 12T\n\
     061999004PULLY     CAST IRON\n\
     05 450120GEAR      SPUR 20T\n\
     04       BELT 1450 30  V-BELT A40\n\
     06       PULLY       DOUBLE GROOVE\n\
     05       BELT  725  8  FLAT 25MM\n\
     05       BELT 2100 15  TIMING\n\
     07 990  1CHAIN     ROLLER\n\
     END DATA.\n";

#[test]
fn reread_reads_a_record_by_its_kind() {
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /SHELF 1-2 KIND 10-14 (A).\n\
         DO IF (KIND EQ 'GEAR').\n\
         REREAD.\n\
         DATA LIST /COST 3-6 (DOLLAR,2) STOCK 7-9.\n\
         END CASE.\n\
         ELSE IF (KIND EQ 'BELT').\n\
         REREAD.\n\
         DATA LIST /COST 15-18 (DOLLAR,2) STOCK 19-21.\n\
         END CASE.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         {PARTS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["SHELF", "KIND", "COST", "STOCK"]);
    assert_eq!(whole(data, "SHELF"), [4.0, 5.0, 4.0, 5.0, 5.0]);
    assert_eq!(
        strings(data, "KIND"),
        ["GEAR", "GEAR", "BELT", "BELT", "BELT"]
    );
    assert_eq!(whole(data, "COST"), [8.75, 4.5, 14.5, 7.25, 21.0]);
    assert_eq!(whole(data, "STOCK"), [12.0, 120.0, 30.0, 8.0, 15.0]);
    assert_eq!(data.variable("COST").unwrap().format(), "DOLLAR6.2");
}

#[test]
fn reread_takes_its_column_from_an_expression() {
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST /KIND 10-14 (A).\n\
         COMPUTE #FROM=1.\n\
         IF (KIND EQ 'BELT') #FROM=13.\n\
         DO IF ANY(KIND, 'BELT', 'GEAR').\n\
         REREAD COLUMN #FROM.\n\
         DATA LIST /COST 3-6 (DOLLAR,2) STOCK 7-9.\n\
         END CASE.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         {PARTS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["KIND", "COST", "STOCK"]);
    assert_eq!(whole(data, "COST"), [8.75, 4.5, 14.5, 7.25, 21.0]);
    assert_eq!(whole(data, "STOCK"), [12.0, 120.0, 30.0, 8.0, 15.0]);
}

#[test]
fn of_several_rereads_the_last_counts() {
    let (s, _) = ran("INPUT PROGRAM.\n\
         DATA LIST NOTABLE /LINE 1-15(A).\n\
         REREAD COLUMN = 4.\n\
         REREAD COLUMN = 9.\n\
         DATA LIST NOTABLE /CODE 1-2(A).\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         10 aa   pq\n\
         20 bb   rs\n\
         30 cc   tu\n\
         END DATA.\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "LINE"),
        ["10 aa   pq", "20 bb   rs", "30 cc   tu"]
    );
    assert_eq!(strings(data, "CODE"), ["pq", "rs", "tu"]);
}

#[test]
fn reread_reads_the_rest_of_a_fixed_record_as_a_list() {
    // a tag in columns 1-10, then three numbers in list form
    let (s, result) = ran("INPUT PROGRAM.\n\
         DATA LIST NOTABLE FIXED /TAG 1-10(A).\n\
         REREAD COLUMN = 11.\n\
         DATA LIST LIST /P Q R.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         alpha one 4 8 15 16\n\
         beta      7 1\n\
         gamma     2 9 3\n\
         delta\n\
         zeta-final6 7 8\n\
         END DATA.\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "TAG"),
        ["alpha one", "beta", "gamma", "delta", "zeta-final"]
    );
    assert_eq!(
        numbers(data, "P"),
        [Some(4.0), Some(7.0), Some(2.0), None, Some(6.0)]
    );
    assert_eq!(
        numbers(data, "Q"),
        [Some(8.0), Some(1.0), Some(9.0), None, Some(7.0)]
    );
    assert_eq!(
        numbers(data, "R"),
        [Some(15.0), None, Some(3.0), None, Some(8.0)]
    );
    // the missing value on the beta record and the three on the delta record are named
    let warned = warnings(&result);
    assert!(warned.iter().any(|w| w.contains("line 8")), "{warned:?}");
    assert!(warned.iter().any(|w| w.contains("line 10")), "{warned:?}");
}

/// Truck loads: the truck in 1-3, its drivers in 5 and the number of loads in 7-8, then from
/// column 10 one group of 9 columns per load: the cargo, a grade letter and the crates.
const TRUCKS: &str = "BEGIN DATA\n\
     101 2 02 APPLESA12PEARS B05\n\
     102 1 01 PLUMS C30\n\
     103 3 03 GRAPESA07CHERRYB11MELONSA04\n\
     END DATA.\n";

#[test]
fn repeating_data_makes_a_case_per_load() {
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST / TRUCK 1-3 DRIVERS 5 NLOAD 7-8.\n\
         REPEATING DATA STARTS=10 /OCCURS=NLOAD\n\
          /DATA=CARGO 1-6 (A) GRADE 7 (A) CRATES 8-9.\n\
         END INPUT PROGRAM.\n\
         {TRUCKS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        names(data),
        ["TRUCK", "DRIVERS", "NLOAD", "CARGO", "GRADE", "CRATES"]
    );
    assert_eq!(
        whole(data, "TRUCK"),
        [101.0, 101.0, 102.0, 103.0, 103.0, 103.0]
    );
    assert_eq!(whole(data, "DRIVERS"), [2.0, 2.0, 1.0, 3.0, 3.0, 3.0]);
    assert_eq!(whole(data, "NLOAD"), [2.0, 2.0, 1.0, 3.0, 3.0, 3.0]);
    assert_eq!(
        strings(data, "CARGO"),
        ["APPLES", "PEARS", "PLUMS", "GRAPES", "CHERRY", "MELONS"]
    );
    assert_eq!(strings(data, "GRADE"), ["A", "B", "C", "A", "B", "A"]);
    assert_eq!(whole(data, "CRATES"), [12.0, 5.0, 30.0, 7.0, 11.0, 4.0]);
}

#[test]
fn occurs_and_length_limit_what_is_read() {
    // one group per record however many there are
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST / TRUCK 1-3 DRIVERS 5 NLOAD 7-8.\n\
         REPEATING DATA STARTS=10 /OCCURS=1\n\
          /DATA=CARGO 1-6 (A) GRADE 7 (A) CRATES 8-9.\n\
         END INPUT PROGRAM.\n\
         {TRUCKS}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "CARGO"), ["APPLES", "PLUMS", "GRAPES"]);
    // only CARGO of groups 9 columns long
    let (s, _) = ran(&format!(
        "INPUT PROGRAM.\n\
         DATA LIST / TRUCK 1-3 DRIVERS 5 NLOAD 7-8.\n\
         REPEATING DATA STARTS=10 /OCCURS=NLOAD /LENGTH=9 /NOTABLE\n\
          /DATA=CARGO 1-6 (A).\n\
         END INPUT PROGRAM.\n\
         {TRUCKS}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["TRUCK", "DRIVERS", "NLOAD", "CARGO"]);
    assert_eq!(
        strings(data, "CARGO"),
        ["APPLES", "PEARS", "PLUMS", "GRAPES", "CHERRY", "MELONS"]
    );
}

#[test]
fn repeating_groups_can_start_in_a_column_a_variable_gives() {
    let (s, _) = ran("INPUT PROGRAM.\n\
         DATA LIST / TRUCK 1-3 DRIVERS 5 NLOAD 7-8.\n\
         DO IF (TRUCK LT 200).\n\
         COMPUTE FIRST=10.\n\
         ELSE.\n\
         COMPUTE FIRST=13.\n\
         END IF.\n\
         REPEATING DATA STARTS=FIRST /OCCURS=NLOAD\n\
           /DATA=CARGO 1-6 (A) GRADE 7 (A) CRATES 8-9.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         101 2 02 APPLESA12PEARS B05\n\
         204 1 01 ...PLUMS C30\n\
         END DATA.");
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "CARGO"), ["APPLES", "PEARS", "PLUMS"]);
    assert_eq!(whole(data, "FIRST"), [10.0, 10.0, 13.0]);
}

#[test]
fn transformations_after_the_program_work_on_its_cases() {
    // a nursery bed, the survey year and its number of trees, then per tree the years it
    // was planted and first pruned
    let (s, _) = ran("INPUT PROGRAM.\n\
         DATA LIST / BED 1 SURVEY 3-6 NTREE 8.\n\
         REPEATING DATA STARTS=9 /OCCURS=NTREE\n\
          /DATA=PLANTED 2-5 PRUNED 7-10.\n\
         END INPUT PROGRAM.\n\
         COMPUTE AGE=SURVEY - PLANTED.\n\
         COMPUTE PRUNEAGE=PRUNED - PLANTED.\n\
         COMPUTE TREE=1.\n\
         IF ($CASENUM GT 1 AND BED EQ LAG(BED)) TREE=LAG(TREE) + 1.\n\
         FORMAT AGE PRUNEAGE TREE (F2).\n\
         BEGIN DATA\n\
         1 2020 2 2012 2015 2014 2016\n\
         2 2021 1 2010 2018\n\
         3 2021 3 2015 2017 2016 2019 2018 2020\n\
         4 2022 1 2019 2021\n\
         END DATA.\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "BED"), [1.0, 1.0, 2.0, 3.0, 3.0, 3.0, 4.0]);
    assert_eq!(
        whole(data, "PLANTED"),
        [2012.0, 2014.0, 2010.0, 2015.0, 2016.0, 2018.0, 2019.0]
    );
    assert_eq!(whole(data, "AGE"), [8.0, 6.0, 11.0, 6.0, 5.0, 3.0, 3.0]);
    assert_eq!(whole(data, "PRUNEAGE"), [3.0, 2.0, 8.0, 2.0, 3.0, 2.0, 2.0]);
    assert_eq!(whole(data, "TREE"), [1.0, 2.0, 1.0, 1.0, 2.0, 3.0, 1.0]);
}

/// The invoice lines of the CONTINUED tests, as they come out.
fn invoices(data: &Dataset) {
    assert_eq!(names(data), ["INVOICE", "NLINES", "PART", "QTY", "UNIT"]);
    assert_eq!(
        whole(data, "INVOICE"),
        [
            31415.0, 31415.0, 31415.0, 31415.0, 31415.0, 31415.0, 31415.0, 27182.0, 27182.0,
            27182.0, 27182.0
        ]
    );
    assert_eq!(
        whole(data, "QTY"),
        [2.0, 12.0, 150.0, 40.0, 1.0, 6.0, 30.0, 8.0, 20.0, 3.0, 100.0]
    );
    assert_eq!(
        whole(data, "UNIT"),
        [4.8, 19.95, 0.35, 1.1, 249.0, 38.5, 7.25, 12.4, 3.15, 88.0, 0.99]
    );
    assert_eq!(strings(data, "PART")[6], "R2-415-10");
    assert_eq!(data.variable("UNIT").unwrap().format(), "DOLLAR10.2");
}

/// The invoice lines: part, quantity and unit price.
const LINES: [(&str, &str, &str); 11] = [
    ("K7-220-01", "002", "4.80"),
    ("K7-220-05", "012", "19.95"),
    ("M3-118-40", "150", "0.35"),
    ("M3-118-41", "040", "1.10"),
    ("P0-900-77", "001", "249.00"),
    ("P0-900-78", "006", "38.50"),
    ("R2-415-10", "030", "7.25"),
    ("A1-001-02", "008", "12.40"),
    ("A1-001-03", "020", "3.15"),
    ("B5-660-90", "003", "88.00"),
    ("B5-660-91", "100", "0.99"),
];

/// Records holding the lines of `LINES` in groups of 21 columns (the price right-aligned in
/// 7, then a blank), so many per record as `per_record` says: the first record of an invoice
/// starts with the invoice and its count of lines, the others with the invoice.
fn invoice_records(per_record: &[&[usize]]) -> String {
    let mut text = String::new();
    for groups in per_record {
        let invoice = if groups[0] < 7 { "31415" } else { "27182" };
        let first = groups[0] == 0 || groups[0] == 7;
        text.push_str(invoice);
        text.push_str(if first {
            if invoice == "31415" { " 07 " } else { " 04 " }
        } else {
            " "
        });
        for &g in *groups {
            let (part, quantity, price) = LINES[g];
            text.push_str(&format!("{part} {quantity}{price:>7} "));
        }
        text.push('\n');
    }
    text
}

#[test]
fn continued_groups_go_on_the_next_records() {
    let program = |starts: &str, continued: &str, data: &str| {
        format!(
            "INPUT PROGRAM.\n\
             DATA LIST / INVOICE 1-5 NLINES 7-8.\n\
             REPEATING DATA STARTS={starts} /OCCURS=NLINES /CONTINUED={continued}\n\
              /DATA=PART 1-9 (A) QTY 11-13 UNIT (DOLLAR7.2,1X).\n\
             END INPUT PROGRAM.\n\
             BEGIN DATA\n{data}END DATA.\n\
             LIST."
        )
    };
    // three groups fit before column 80
    let records = invoice_records(&[&[0, 1, 2], &[3, 4, 5], &[6], &[7, 8, 9], &[10]]);
    let (s, _) = ran(&program("10", "7", &records));
    invoices(s.dataset().unwrap());
    // two groups on the first record, three on the others
    let records = invoice_records(&[&[0, 1], &[2, 3, 4], &[5, 6], &[7, 8], &[9, 10]]);
    let (s, _) = ran(&program("10-55", "7", &records));
    invoices(s.dataset().unwrap());
    // two groups on every record
    let records = invoice_records(&[&[0, 1], &[2, 3], &[4, 5], &[6], &[7, 8], &[9, 10]]);
    let (s, _) = ran(&program("10-55", "7-55", &records));
    invoices(s.dataset().unwrap());
}

#[test]
fn id_checks_the_continuation_records() {
    let program = |data: &str| {
        format!(
            "INPUT PROGRAM.\n\
             DATA LIST / INVOICE 1-5 NLINES 7-8.\n\
             REPEATING DATA STARTS=10-50 /OCCURS=NLINES\n\
              /CONTINUED=7 /ID=1-5=INVOICE\n\
              /DATA=PART 1-9 (A) QTY 11-13 UNIT 15-20 (2).\n\
             END INPUT PROGRAM.\n\
             BEGIN DATA\n{data}END DATA.\n\
             LIST."
        )
    };
    let records = "31415 04 K7-220-01 002   4.80K7-220-05 012  19.95\n\
                   31415 M3-118-40 150   0.35M3-118-41 040   1.10\n\
                   27182 03 A1-001-02 008  12.40A1-001-03 020   3.15\n\
                   27182 B5-660-90 003  88.00\n";
    let (s, _) = ran(&program(records));
    let data = s.dataset().unwrap();
    assert_eq!(
        strings(data, "PART"),
        [
            "K7-220-01",
            "K7-220-05",
            "M3-118-40",
            "M3-118-41",
            "A1-001-02",
            "A1-001-03",
            "B5-660-90"
        ]
    );
    assert_eq!(
        whole(data, "UNIT"),
        [4.8, 19.95, 0.35, 1.1, 12.4, 3.15, 88.0]
    );
    let wrong = records.replace("31415 M3", "31416 M3");
    let message = fails(&program(&wrong));
    assert!(message.contains("not 31415"), "{message}");
}

/// Club records: the type in 1-2, then team, age, minutes played and goals. Types 11-14 have
/// the minutes in 7-9 and the goals in 11, type 15 in 9-11 and 14, the others in 16-18 and 22.
const CLUB: &str = "BEGIN DATA\n\
     11 216090 2\n\
     12 129045 0\n\
     15 131  075  1\n\
     40 244         060   3\n\
     13 217120 1\n\
     14 325030 0\n\
     11 118064 1\n\
     15 238  110  2\n\
     41 352         015   0\n\
     40 147         088   1\n\
     END DATA.\n";

#[test]
fn a_mixed_file_reads_the_listed_record_types() {
    let (s, result) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2.\n\
         RECORD TYPE 13.\n\
         DATA LIST   /TEAM 4 AGE 5-6 MINUTES 7-9 GOALS 11.\n\
         END FILE TYPE.\n\
         {CLUB}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["RECID", "TEAM", "AGE", "MINUTES", "GOALS"]);
    assert_eq!(whole(data, "RECID"), [13.0]);
    assert_eq!(whole(data, "TEAM"), [2.0]);
    assert_eq!(whole(data, "AGE"), [17.0]);
    assert_eq!(whole(data, "MINUTES"), [120.0]);
    assert_eq!(whole(data, "GOALS"), [1.0]);
    // a mixed file skips other types without a warning
    assert!(warnings(&result).is_empty(), "{:?}", warnings(&result));

    let (s, _) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2.\n\
         + RECORD TYPE 11,12,13,14.\n\
         + DATA LIST   /TEAM 4 AGE 5-6 MINUTES 7-9 GOALS 11.\n\
         + RECORD TYPE 15.\n\
         + DATA LIST   /TEAM 4 AGE 5-6 MINUTES 9-11 GOALS 14.\n\
         END FILE TYPE.\n\
         {CLUB}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        whole(data, "RECID"),
        [11.0, 12.0, 15.0, 13.0, 14.0, 11.0, 15.0]
    );
    assert_eq!(
        whole(data, "MINUTES"),
        [90.0, 45.0, 75.0, 120.0, 30.0, 64.0, 110.0]
    );
    assert_eq!(
        whole(data, "AGE"),
        [16.0, 29.0, 31.0, 17.0, 25.0, 18.0, 38.0]
    );
    assert_eq!(whole(data, "GOALS"), [2.0, 0.0, 1.0, 1.0, 0.0, 1.0, 2.0]);

    // OTHER reads every other type with its own columns
    let (s, _) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2 WILD=NOWARN.\n\
         RECORD TYPE 11,12,13,14.\n\
         DATA LIST   /TEAM 4 AGE 5-6 MINUTES 7-9 GOALS 11.\n\
         RECORD TYPE 15.\n\
         DATA LIST   /TEAM 4 AGE 5-6 MINUTES 9-11 GOALS 14.\n\
         RECORD TYPE OTHER.\n\
         DATA LIST   /TEAM 4 AGE 5-6 MINUTES 16-18 GOALS 22.\n\
         END FILE TYPE.\n\
         {CLUB}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        whole(data, "RECID"),
        [11.0, 12.0, 15.0, 40.0, 13.0, 14.0, 11.0, 15.0, 41.0, 40.0]
    );
    assert_eq!(whole(data, "AGE")[3], 44.0);
    assert_eq!(whole(data, "MINUTES")[3], 60.0);
    assert_eq!(whole(data, "GOALS")[9], 1.0);
}

#[test]
fn wild_warn_names_the_undefined_records() {
    let (_, result) = ran(&format!(
        "FILE TYPE MIXED RECORD=RECID 1-2 WILD=WARN.\n\
         RECORD TYPE 11,12,13,14,15.\n\
         DATA LIST /TEAM 4.\n\
         END FILE TYPE.\n{CLUB}"
    ));
    let warned = warnings(&result);
    assert_eq!(warned.len(), 3, "{warned:?}");
    assert!(
        warned[0].contains("record type 40 is not defined: 40 244"),
        "{warned:?}"
    );
}

/// Runners with three records each: 1 the club, 2 a sprint time and 3 a distance.
const RUNNERS: &str = "BEGIN DATA\n\
     0101 1 TK\n\
     0101 2 125\n\
     0101 3 310\n\
     0102 1 VS\n\
     0102 2 118\n\
     0102 3 402\n\
     0103 1 TK\n\
     0103 2 131\n\
     0103 3 288\n\
     0104 1 PL\n\
     0104 2 109\n\
     0104 3 365\n\
     END DATA.\n";

const GROUPED: &str = "RECORD TYPE 1.\n\
     DATA LIST /CLUB 8-9 (A).\n\
     RECORD TYPE 2.\n\
     DATA LIST /SPRINT 8-10.\n\
     RECORD TYPE 3.\n\
     DATA LIST /DISTANCE 8-10.\n\
     END FILE TYPE.\n";

#[test]
fn a_grouped_file_makes_one_case_per_runner() {
    let (s, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#KIND 6 CASE=RUNNER 1-4.\n{GROUPED}{RUNNERS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["RUNNER", "CLUB", "SPRINT", "DISTANCE"]);
    assert_eq!(whole(data, "RUNNER"), [101.0, 102.0, 103.0, 104.0]);
    assert_eq!(strings(data, "CLUB"), ["TK", "VS", "TK", "PL"]);
    assert_eq!(whole(data, "SPRINT"), [125.0, 118.0, 131.0, 109.0]);
    assert_eq!(whole(data, "DISTANCE"), [310.0, 402.0, 288.0, 365.0]);
    assert!(warnings(&result).is_empty());
}

#[test]
fn a_grouped_file_with_missing_and_unordered_records() {
    let (s, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#KIND 6 CASE=RUNNER 1-4 MISSING=NOWARN.\n{GROUPED}\
         BEGIN DATA\n\
         0101 1 TK\n\
         0101 2 125\n\
         0101 3 310\n\
         0102 1 VS\n\
         0102 3 402\n\
         0103 2 131\n\
         0104 1 PL\n\
         0104 2 109\n\
         0104 3 365\n\
         END DATA.\n"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "RUNNER"), [101.0, 102.0, 103.0, 104.0]);
    assert_eq!(strings(data, "CLUB"), ["TK", "VS", "", "PL"]);
    assert_eq!(
        numbers(data, "SPRINT"),
        [Some(125.0), None, Some(131.0), Some(109.0)]
    );
    assert_eq!(
        numbers(data, "DISTANCE"),
        [Some(310.0), Some(402.0), None, Some(365.0)]
    );
    assert!(warnings(&result).is_empty());

    let (s, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#KIND 6 CASE=RUNNER 1-4 MISSING=NOWARN\n\
           ORDERED=NO.\n{GROUPED}\
         BEGIN DATA\n\
         0101 3 310\n\
         0101 1 TK\n\
         0102 2 118\n\
         0102 3 402\n\
         0102 1 VS\n\
         0103 3 288\n\
         0104 1 PL\n\
         0104 3 365\n\
         0104 2 109\n\
         END DATA.\n"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "CLUB"), ["TK", "VS", "", "PL"]);
    assert_eq!(
        numbers(data, "SPRINT"),
        [None, Some(118.0), None, Some(109.0)]
    );
    assert_eq!(
        whole(data, "DISTANCE"),
        [310.0, 402.0, 288.0, 365.0]
    );
    assert!(warnings(&result).is_empty());

    // the defaults warn about missing, duplicate and unordered records
    let (_, result) = ran(&format!(
        "FILE TYPE GROUPED RECORD=#KIND 6 CASE=RUNNER 1-4.\n{GROUPED}\
         BEGIN DATA\n\
         0101 3 310\n\
         0101 1 TK\n\
         0101 1 TV\n\
         0102 1 VS\n\
         0102 7 XX\n\
         END DATA.\n"
    ));
    let warned = warnings(&result).join("\n");
    assert!(warned.contains("out of the order"), "{warned}");
    assert!(warned.contains("a second record of this type"), "{warned}");
    assert!(
        warned.contains("case 101 has no record of type 2"),
        "{warned}"
    );
    assert!(
        warned.contains("case 102 has no record of type 3"),
        "{warned}"
    );
    assert!(warned.contains("record type 7 is not defined"), "{warned}");
}

const NESTED: &str = "RECORD TYPE 1.\n\
     DATA LIST /FARM_NO 8-10 REGION 12 COUNTY 14-15 (A) VISIT 17-24 (A).\n\
     RECORD TYPE 2.\n\
     DATA LIST /FIELD 8 SOIL 10 SLOPE 11-12 OWNER 14-15 (A) TENURE 17-20 (A).\n\
     RECORD TYPE 3.\n\
     DATA LIST /PLOT 8 MOIST 10-11 CROP 13 (A) PEST 15 ROW 17-18 (A)\n\
                YIELD 20-22.\n\
     END FILE TYPE.\n";

/// One farm visit (type 1) with three fields (type 2) and their sample plots (type 3).
const FARMS: &str = "BEGIN DATA\n\
     0007 1 412 3 HA 05/06/24\n\
     0007 2 1 412 RE LONG\n\
     0007 3 1 23 W 0 N1 450\n\
     0007 2 2 205 OW NONE\n\
     0007 3 1 31 B 1 N2 380\n\
     0007 3 2 28 B 0 S1 410\n\
     0007 3 3 19 W 2 S2 295\n\
     0007 2 3 520 RE SHRT\n\
     0007 3 1 26 O 0 E1 120\n\
     END DATA.\n";

#[test]
fn a_nested_file_makes_a_case_per_plot() {
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=SITE 1-4.\n{NESTED}{FARMS}LIST."
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "SITE", "FARM_NO", "REGION", "COUNTY", "VISIT", "FIELD", "SOIL", "SLOPE", "OWNER",
            "TENURE", "PLOT", "MOIST", "CROP", "PEST", "ROW", "YIELD"
        ]
    );
    assert_eq!(whole(data, "SITE"), [7.0; 5]);
    assert_eq!(whole(data, "FARM_NO"), [412.0; 5]);
    assert_eq!(strings(data, "COUNTY"), ["HA"; 5]);
    assert_eq!(strings(data, "VISIT"), ["05/06/24"; 5]);
    assert_eq!(whole(data, "FIELD"), [1.0, 2.0, 2.0, 2.0, 3.0]);
    assert_eq!(whole(data, "SLOPE"), [12.0, 5.0, 5.0, 5.0, 20.0]);
    assert_eq!(strings(data, "OWNER"), ["RE", "OW", "OW", "OW", "RE"]);
    assert_eq!(whole(data, "PLOT"), [1.0, 1.0, 2.0, 3.0, 1.0]);
    assert_eq!(whole(data, "MOIST"), [23.0, 31.0, 28.0, 19.0, 26.0]);
    assert_eq!(strings(data, "ROW"), ["N1", "N2", "S1", "S2", "E1"]);
    assert_eq!(whole(data, "YIELD"), [450.0, 380.0, 410.0, 295.0, 120.0]);
}

#[test]
fn spread_no_gives_the_field_to_its_first_plot_only() {
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=#RECID 6 CASE=SITE 1-4.\n{}{FARMS}",
        NESTED.replace("RECORD TYPE 2.", "RECORD TYPE 2 SPREAD=NO.")
    ));
    let data = s.dataset().unwrap();
    assert_eq!(
        numbers(data, "FIELD"),
        [Some(1.0), Some(2.0), None, None, Some(3.0)]
    );
    assert_eq!(strings(data, "OWNER"), ["RE", "OW", "", "", "RE"]);
    assert_eq!(whole(data, "FARM_NO"), [412.0; 5]);
}

#[test]
fn duplicate_and_missing_records_of_a_nested_file() {
    // a field record with no plot below it, just before the record of field 2
    let duplicated = FARMS.replace(
        "0007 2 2 205 OW NONE",
        "0007 2 9 330 TE LONG\n0007 2 2 205 OW NONE",
    );
    let (s, result) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=SITE 1-4 DUPLICATE=WARN.\n{NESTED}{duplicated}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 5);
    assert_eq!(strings(data, "OWNER"), ["RE", "OW", "OW", "OW", "RE"]);
    let warned = warnings(&result);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("0007 2 2 205 OW NONE"), "{warned:?}");

    // CASE: the empty field is a case of its own
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=SITE 1-4 DUPLICATE=CASE.\n{NESTED}{duplicated}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 6);
    assert_eq!(
        strings(data, "OWNER"),
        ["RE", "TE", "OW", "OW", "OW", "RE"]
    );
    assert_eq!(
        numbers(data, "MOIST"),
        [
            Some(23.0),
            None,
            Some(31.0),
            Some(28.0),
            Some(19.0),
            Some(26.0)
        ]
    );

    // a plot without a field record
    let (s, result) = ran(&format!(
        "FILE TYPE NESTED RECORD=6 CASE=SITE 1-4 MISSING=WARN.\n{NESTED}{}",
        FARMS.replace("0007 2 1 412 RE LONG\n", "")
    ));
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 5);
    assert_eq!(numbers(data, "FIELD")[0], None);
    assert_eq!(numbers(data, "MOIST")[0], Some(23.0));
    let warned = warnings(&result);
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert!(warned[0].contains("no record of type 2"), "{warned:?}");
}

#[test]
fn other_skip_makes_the_field_the_lowest_level() {
    let (s, _) = ran(&format!(
        "FILE TYPE NESTED RECORD=#RECID 6 CASE=SITE 1-4.\n\
         RECORD TYPE 1.\n\
         DATA LIST /REGION 12.\n\
         RECORD TYPE 2.\n\
         DATA LIST /SLOPE 11-12.\n\
         RECORD TYPE OTHER SKIP.\n\
         END FILE TYPE.\n{FARMS}"
    ));
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["SITE", "REGION", "SLOPE"]);
    assert_eq!(data.case_count(), 3);
    assert_eq!(whole(data, "REGION"), [3.0; 3]);
    assert_eq!(whole(data, "SLOPE"), [12.0, 5.0, 20.0]);
}

#[test]
fn repeating_data_reads_the_records_of_a_mixed_file() {
    // only the loads of truck 102; END FILE stops at its record, so the later one is not read
    let (s, _) = ran("FILE TYPE MIXED RECORD=#T 1-3.\n\
         RECORD TYPE 102.\n\
         REPEATING DATA STARTS=10 /OCCURS=3\n\
          /DATA=CARGO 1-6(A) GRADE 7(A) CRATES 8-9.\n\
         END FILE.\n\
         END FILE TYPE.\n\
         BEGIN DATA\n\
         101 2 02 APPLESA12PEARS B05\n\
         102 3 03 GRAPESA07CHERRYB11MELONS\n\
         103 1 01 PLUMS C30\n\
         102 1 01 PEARS A09\n\
         END DATA.\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["CARGO", "GRADE", "CRATES"]);
    assert_eq!(strings(data, "CARGO"), ["GRAPES", "CHERRY", "MELONS"]);
    assert_eq!(strings(data, "GRADE"), ["A", "B", ""]);
    assert_eq!(numbers(data, "CRATES"), [Some(7.0), Some(11.0), None]);
}

#[test]
fn leave_keeps_a_running_total() {
    let source = "DATA LIST LIST /Visits Spend Tips.\n\
         BEGIN DATA\n\
         2 30 4\n\
         1 12 0\n\
         3 45 6\n\
         END DATA.\n\
         COMPUTE RunVisits=RunVisits+Visits.\n\
         LEAVE RunVisits.\n";
    let (mut s, _) = ran(&format!("{source}LIST."));
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "RunVisits"), [2.0, 3.0, 6.0]);
    // a variable that existed before cannot be left
    let message = fails(&format!(
        "{source}COMPUTE Spend=Spend*2.\n\
         LEAVE Spend."
    ));
    assert!(message.contains("Spend existed before"), "{message}");
    // LEAVE before the command that defines the variable
    let message = fails(&format!(
        "{source}LEAVE RunTips.\n\
         COMPUTE RunTips=RunTips+Tips."
    ));
    assert!(message.contains("RunTips"), "{message}");
    // a value set for some cases only is carried to the next ones
    run(
        &mut s,
        "IF (Visits = 1) bonus = 5.\nLEAVE bonus.\nSTRING tag (A3).\nIF (Spend < 20) tag = 'lo'.\nLEAVE tag.\nEXECUTE.",
    );
    let data = s.dataset().unwrap();
    assert_eq!(whole(data, "bonus"), [0.0, 5.0, 5.0]);
    assert_eq!(strings(data, "tag"), ["", "lo", "lo"]);
}

#[test]
fn leave_with_lag_sums_per_group() {
    let (s, _) = ran("DATA LIST LIST /TEAM POINTS.\n\
         BEGIN DATA\n\
         3 12\n\
         1 7\n\
         3 9\n\
         1 4\n\
         2 10\n\
         1 1\n\
         END DATA.\n\
         SORT CASES TEAM.\n\
         IF (TEAM NE LAG(TEAM)) RUNPTS=0.\n\
         COMPUTE RUNPTS=RUNPTS+POINTS.\n\
         LEAVE RUNPTS.\n\
         FORMATS RUNPTS (COMMA6)/ POINTS (F3).\n\
         LIST.");
    let data = s.dataset().unwrap();
    assert_eq!(
        whole(data, "RUNPTS"),
        [7.0, 11.0, 12.0, 10.0, 12.0, 21.0]
    );
    assert!(
        data.variable("RUNPTS")
            .unwrap()
            .format()
            .starts_with("COMMA6")
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
