//! Input programs, REREAD, LEAVE and MATRIX DATA compared with GNU PSPP, run as a black box on the same
//! syntax: the data files both programs save must hold the same variables, formats and
//! values. PSPP has no REPEATING DATA or FILE TYPE, so those are checked by hand only
//! (tests/input_program.rs). Ignored by default because they need the PSPP program:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_input -- --ignored
//! ```

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

fn folder(case: &str, who: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "statscraft-reference-input-{}-{case}-{who}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `job` in both programs, each in a folder of its own that holds `files`, and compares
/// the data files they save.
fn compare(case: &str, job: &str, files: &[(&str, &str)]) {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return;
    };
    let job = format!("{job}\nSAVE OUTFILE='out.sav'.\n");
    let theirs = folder(case, "pspp");
    let ours = folder(case, "ours");
    for dir in [&theirs, &ours] {
        for (name, text) in files {
            fs::write(dir.join(name), text).unwrap();
        }
    }
    fs::write(theirs.join("job.sps"), &job).unwrap();
    // PSPP is a Windows program on the development machine: relative paths only
    let status = Command::new(&pspp)
        .args(["job.sps", "-o", "pspp.csv", "-O", "format=csv"])
        .current_dir(&theirs)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(&pspp).display()));
    let log = fs::read_to_string(theirs.join("pspp.csv")).unwrap_or_default();
    assert!(status.success(), "{case}: PSPP failed\n{log}");

    let mut session = Session::default();
    session
        .run(&format!("CD '{}'.\n{job}", ours.display()), None)
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
    same_data(case, &theirs.join("out.sav"), &ours.join("out.sav"));
    fs::remove_dir_all(theirs).unwrap();
    fs::remove_dir_all(ours).unwrap();
}

fn same_data(case: &str, theirs: &Path, ours: &Path) {
    let (a, b) = (
        Dataset::read_sav(theirs).unwrap(),
        Dataset::read_sav(ours).unwrap(),
    );
    let names = |d: &Dataset| d.variable_names().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(names(&b), names(&a), "{case}: variables");
    assert_eq!(b.case_count(), a.case_count(), "{case}: cases");
    for name in names(&a) {
        let (x, y) = (a.variable(&name).unwrap(), b.variable(&name).unwrap());
        assert_eq!(y.format(), x.format(), "{case}: format of {name}");
        match (x.values(), y.values()) {
            (Values::Numeric(x), Values::Numeric(y)) => {
                assert_eq!(
                    y.iter().collect::<Vec<_>>(),
                    x.iter().collect::<Vec<_>>(),
                    "{case}: {name}"
                )
            }
            (Values::String(x), Values::String(y)) => {
                assert_eq!(
                    y.iter().collect::<Vec<_>>(),
                    x.iter().collect::<Vec<_>>(),
                    "{case}: {name}"
                )
            }
            _ => panic!("{case}: {name} has another type"),
        }
    }
}

#[test]
#[ignore = "needs PSPP"]
fn end_case_after_a_loop() {
    // after END CASE inside the loop, GNU PSPP resets the scratch variables we keep
    compare(
        "items",
        "INPUT PROGRAM.\n\
         DATA LIST /#S1 TO #S4 (4(F1,1X)).\n\
         VECTOR SCORE=#S1 TO #S4.\n\
         COMPUTE PTS=0.\n\
         LOOP #K=1 TO 4.\n\
         COMPUTE PTS=PTS+SCORE(#K).\n\
         END LOOP.\n\
         END CASE.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         4 0 7 2\n\
         9 3 3 8\n\
         END DATA.\n\
         FORMATS PTS (F2.0).",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn one_case_per_run_without_end_case() {
    compare(
        "headers",
        "INPUT PROGRAM.\n\
         DATA LIST /#N 1 BLDG LEVEL 3-4(A).\n\
         LOOP #I=1 TO #N.\n\
         DATA LIST /TEMP 1-3(1).\n\
         END LOOP.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
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
         END DATA.",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn end_file_with_and_without_end_case() {
    let meter = "01 1  4520\n02 2  4877\n03 1  5012\n04 2  5190\n05 1  5333\n";
    compare(
        "end-file",
        "INPUT PROGRAM.\n\
         DATA LIST FILE='meter.txt' /DAY 1-2 SHIFT 4 KWH 6-10(1).\n\
         DO IF (DAY GE 4).\n\
         END FILE.\n\
         END IF.\n\
         END INPUT PROGRAM.",
        &[("meter.txt", meter)],
    );
    compare(
        "end-file-case",
        "INPUT PROGRAM.\n\
         DATA LIST FILE='meter.txt' /DAY 1-2 SHIFT 4 KWH 6-10(1).\n\
         DO IF (DAY GE 4).\n\
         END CASE.\n\
         END FILE.\n\
         ELSE.\n\
         END CASE.\n\
         END IF.\n\
         END INPUT PROGRAM.",
        &[("meter.txt", meter)],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn a_generated_index() {
    // the program that skips its first records differs: see docs/differences.md
    compare(
        "generated",
        "INPUT PROGRAM.\n\
         LOOP K=3 TO 30 BY 3.\n\
         COMPUTE CUBE=K**3.\n\
         END CASE.\n\
         END LOOP.\n\
         END FILE.\n\
         END INPUT PROGRAM.",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn end_variables_join_files() {
    compare(
        "three-files",
        "INPUT PROGRAM.\n\
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
         END INPUT PROGRAM.",
        &[
            ("north.txt", "Harbour      12 TA\nMill Lane     7 TA\n"),
            ("south.txt", "Riverside           9 PA\n"),
            ("east.txt", "  Old Square   15 NA\n  Station       4 TA\n"),
        ],
    );
    // the running total kept with LEAVE differs: see docs/differences.md
}

#[test]
#[ignore = "needs PSPP"]
fn reread_by_kind_and_by_column() {
    compare(
        "reread",
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
         BEGIN DATA\n\
         04 875012GEAR      SPROCKET 12T\n\
         061999004PULLY     CAST IRON\n\
         05 450120GEAR      SPUR 20T\n\
         04       BELT 1450 30  V-BELT A40\n\
         06       PULLY       DOUBLE GROOVE\n\
         05       BELT  725  8  FLAT 25MM\n\
         05       BELT 2100 15  TIMING\n\
         07 990  1CHAIN     ROLLER\n\
         END DATA.",
        &[],
    );
    compare(
        "reread-twice",
        "INPUT PROGRAM.\n\
         DATA LIST NOTABLE /LINE 1-15(A).\n\
         REREAD COLUMN = 4.\n\
         REREAD COLUMN = 9.\n\
         DATA LIST NOTABLE /CODE 1-2(A).\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         10 aa   pq\n\
         20 bb   rs\n\
         30 cc   tu\n\
         END DATA.",
        &[],
    );
    compare(
        "reread-list",
        "INPUT PROGRAM.\n\
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
         END DATA.",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn leave_outside_an_input_program() {
    compare(
        "leave",
        "DATA LIST LIST /Visits Spend Tips.\n\
         BEGIN DATA\n\
         2 30 4\n\
         1 12 0\n\
         3 45 6\n\
         END DATA.\n\
         COMPUTE RunVisits=RunVisits+Visits.\n\
         LEAVE RunVisits.\n\
         IF (Visits = 1) bonus = 5.\n\
         LEAVE bonus.",
        &[],
    );
    compare(
        "leave-lag",
        "DATA LIST LIST /TEAM POINTS.\n\
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
         FORMATS RUNPTS (COMMA6)/ POINTS (F3).",
        &[],
    );
}

/// The layouts of MATRIX DATA, each with its own synthetic numbers.
const MATRIX_CASES: [(&str, &str); 14] = [
    (
        "explicit",
        "MATRIX DATA VARIABLES=ROWTYPE_ a b c.\nBEGIN DATA\n'MEAN' 3.25 41.5 0.82\nSD 1.1 7.4 0.35\nN 60 60 60\nCORR 1\nCORR -.21 1\nCORR .48 -.66 1\nEND DATA.",
    ),
    (
        "upper-nodiagonal",
        "MATRIX DATA VARIABLES=a b c d /CONTENTS=MEAN SD N CORR /FORMAT=UPPER NODIAGONAL.\nBEGIN DATA\n3.2 41.5 0.8 512.4\n1.1 7.4 0.4 88.2\n60 60 60 60\n-.21 .48 .15\n-.66 -.33\n.57\nEND DATA.",
    ),
    (
        "within-together",
        "MATRIX DATA VARIABLES=f a b c /FACTORS=f /CELLS=2 /CONTENTS=(MEAN SD N) CORR.\nBEGIN DATA\n1 10 20 30\n1 1 2 3\n1 15 15 15\n2 11 18 33\n2 1.2 2.5 2.8\n2 12 12 12\n1\n.25 1\n.4 -.3 1\nEND DATA.",
    ),
    (
        "within-apart",
        "MATRIX DATA VARIABLES=f a b c /FACTORS=f /CELLS=2 /CONTENTS=(MEAN) (SD) (N) CORR.\nBEGIN DATA\n1 10 20 30\n2 11 18 33\n1 1 2 3\n2 1.2 2.5 2.8\n1 15 15 15\n2 12 12 12\n1\n.25 1\n.4 -.3 1\nEND DATA.",
    ),
    (
        "two-factors-implicit",
        "MATRIX DATA VARIABLES=f1 f2 a b /FACTORS=f1 f2 /CELLS=4 /CONTENTS=(MEAN SD) CORR.\nBEGIN DATA\n1 1 1 2\n1 1 5 4\n1 2 4 5\n1 2 6 5\n2 1 7 8\n2 1 7 6\n2 2 9 8\n2 2 8 7\n1\n.6 1\nEND DATA.",
    ),
    (
        "explicit-factor",
        "MATRIX DATA VARIABLES=site ROWTYPE_ heat cost /FACTORS=site.\nBEGIN DATA\n1 N 18 18\n1 MEAN 21.4 63.1\n2 N 9 9\n2 MEAN 19.85 70.25\n. SD 3.2 11.7\n. CORR 1\n. CORR -.37 1\nEND DATA.",
    ),
    (
        "mse-dfe",
        "MATRIX DATA VARIABLES=dose ROWTYPE_ gain /FACTORS=dose.\nBEGIN DATA\n1 N 24\n2 N 31\n1 MEAN 4.125\n2 MEAN 5.0484\n. MSE 2.3317\n. DFE 53\nEND DATA.",
    ),
    (
        "split-generated",
        "MATRIX DATA VARIABLES=a b c /CONTENTS=MEAN SD N CORR /SPLIT=spl.\nBEGIN DATA\n12 7 30\n2 1.5 4\n40 40 40\n1\n.25 1\n.4 -.3 1\n14 6 28\n2.5 1 3\n35 35 35\n1\n.1 1\n.2 .15 1\nEND DATA.",
    ),
    (
        "split-in-data",
        "MATRIX DATA VARIABLES=s1 ROWTYPE_ a b c /SPLIT=s1.\nBEGIN DATA\n3 MEAN 12 7 30\n3 SD 2 1.5 4\n3 N 40 40 40\n3 CORR 1\n3 CORR .25 1\n3 CORR .4 -.3 1\n4 MEAN 14 6 28\n4 SD 2.5 1 3\n4 N 35 35 35\n4 CORR 1\n4 CORR .1 1\n4 CORR .2 .15 1\nEND DATA.",
    ),
    (
        "n-subcommand",
        "MATRIX DATA VARIABLES=a b c /CONTENTS=MEAN SD CORR /N=55.\nBEGIN DATA\n12 7 30\n4 3 2.5\n1\n.25 1\n.4 -.3 1\nEND DATA.",
    ),
    (
        "n-scalar",
        "MATRIX DATA VARIABLES=a b c /CONTENTS=MEAN SD N_SCALAR CORR.\nBEGIN DATA\n12 7 30\n2 1.5 4\n40\n1\n.25 1\n.4 -.3 1\nEND DATA.",
    ),
    (
        "full",
        "MATRIX DATA VARIABLES=ROWTYPE_ a b c /FORMAT=FULL.\nBEGIN DATA\nMEAN 12 7 30\nCOV 4 1.2 .5\nCOV 1.2 9 2.1\nCOV .5 2.1 1\nEND DATA.",
    ),
    (
        "lower-nodiagonal",
        "MATRIX DATA VARIABLES=ROWTYPE_ a b c /FORMAT=LOWER NODIAGONAL.\nBEGIN DATA\nN 40 40 40\nCORR .25\nCORR .4 -.3\nEND DATA.",
    ),
    (
        "free-prox",
        "MATRIX DATA VARIABLES=a b c /CONTENTS=PROX /FORMAT=FREE NODIAGONAL.\nBEGIN DATA\n1.5 4.25\n3\nEND DATA.",
    ),
];

#[test]
#[ignore = "needs PSPP"]
fn matrix_data_layouts() {
    for (case, job) in MATRIX_CASES {
        compare(&format!("matrix-{case}"), job, &[]);
    }
}

/// SET settings that change how data are read and computed. PSPP reads a lone period and a
/// blank date field as system-missing whatever BLANKS says, and goes wrong after SET
/// EPOCH=AUTOMATIC, so those cases are checked against the manual only (tests/settings.rs).
const SETTING_CASES: [(&str, &str); 6] = [
    (
        "blanks-fixed",
        "SET BLANKS=-9.\nDATA LIST FIXED /x 1-3 s 4-5 (A).\nBEGIN DATA\n  1ab\n\n  2\nEND DATA.",
    ),
    (
        "blanks-list",
        "SET BLANKS=-9.\nDATA LIST LIST (',') /a b c.\nBEGIN DATA\n1,,3\n4, ,6\nEND DATA.",
    ),
    (
        "blanks-free-number",
        "SET BLANKS=0.\nDATA LIST FREE /a b.\nBEGIN DATA\n1,,3,4\nEND DATA.\n\
         COMPUTE n = NUMBER('   ', F3).",
    ),
    (
        "epoch-1900",
        "SET EPOCH=1900.\nDATA LIST LIST /d (DATE11) a (ADATE8).\nBEGIN DATA\n\
         1-JAN-05 01/01/85\nEND DATA.\nCOMPUTE f = DATE.DMY(1,1,85).\n\
         COMPUTE g = NUMBER('1-JAN-05', DATE9).\nCOMPUTE m = DATE.MOYR(2,30).\n\
         COMPUTE y = YRMODA(85,1,1).",
    ),
    (
        "epoch-1980",
        "SET EPOCH=1980.\nDATA LIST LIST /d (DATE11) a (ADATE8).\nBEGIN DATA\n\
         1-JAN-05 01/01/85\nEND DATA.\nCOMPUTE f = DATE.DMY(1,1,79).\n\
         COMPUTE g = NUMBER('1-JAN-80', DATE9).",
    ),
    (
        "fuzzbits-0",
        "SET FUZZBITS=0.\nDATA LIST LIST /x.\nBEGIN DATA\n0.285\n0.29\nEND DATA.\n\
         COMPUTE r = RND(x * 100).\nCOMPUTE t = TRUNC(x * 100).\n\
         COMPUTE r6 = RND(x * 100, 1, 6).\nCOMPUTE t6 = TRUNC(x * 100, 1, 6).",
    ),
];

#[test]
#[ignore = "needs PSPP"]
fn settings_that_change_results() {
    for (case, job) in SETTING_CASES {
        compare(&format!("set-{case}"), job, &[]);
    }
}
