//! Input programs, REREAD and LEAVE compared with GNU PSPP, run as a black box on the same
//! syntax: the data files both programs save must hold the same variables, formats and
//! values. PSPP has no REPEATING DATA or FILE TYPE, so those are checked against the manual
//! only (tests/input_program.rs). Ignored by default because they need the PSPP program:
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
    // after END CASE inside the loop, GNU PSPP resets the scratch variables the manual keeps
    compare(
        "items",
        "INPUT PROGRAM.\n\
         DATA LIST /#X1 TO #X3 (3(F1,1X)).\n\
         VECTOR V=#X1 TO #X3.\n\
         COMPUTE X=0.\n\
         LOOP #I=1 TO 3.\n\
         COMPUTE X=X+V(#I).\n\
         END LOOP.\n\
         END CASE.\n\
         END INPUT PROGRAM.\n\
         BEGIN DATA\n\
         2 1 1\n\
         3 5 1\n\
         END DATA.\n\
         FORMATS X (F1.0).",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn one_case_per_run_without_end_case() {
    compare(
        "headers",
        [removed]
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn end_file_with_and_without_end_case() {
    let prices = "1878 1 1234\n1879 2 1301\n1880 3 1420\n1881 4 1530\n1882 1 1600\n";
    compare(
        "end-file",
        "INPUT PROGRAM.\n\
         DATA LIST FILE='prices.txt' /YEAR 1-4 QUARTER 6 PRICE 8-12(2).\n\
         DO IF (YEAR GE 1881).\n\
         END FILE.\n\
         END IF.\n\
         END INPUT PROGRAM.",
        &[("prices.txt", prices)],
    );
    compare(
        "end-file-case",
        "INPUT PROGRAM.\n\
         DATA LIST FILE='prices.txt' /YEAR 1-4 QUARTER 6 PRICE 8-12(2).\n\
         DO IF (YEAR GE 1881).\n\
         END CASE.\n\
         END FILE.\n\
         ELSE.\n\
         END CASE.\n\
         END IF.\n\
         END INPUT PROGRAM.",
        &[("prices.txt", prices)],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn a_generated_index() {
    // the skip example of the manual differs: see docs/differences.md
    compare(
        "generated",
        "INPUT PROGRAM.\n\
         LOOP I=1 TO 20.\n\
         COMPUTE SQUARE=I*I.\n\
         END CASE.\n\
         END LOOP.\n\
         END FILE.\n\
         END INPUT PROGRAM.",
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn end_variables_concatenate_files() {
    compare(
        "three-files",
        "INPUT PROGRAM.\n\
         NUMERIC #EOF1 TO #EOF3.\n\
         [removed]
        &[
            (
                "one.txt",
                "Ann                 34 F\nBob                 45 M\n",
            ),
            ("two.txt", "Cy                  29 M\n"),
            (
                "three.txt",
                "Di                      51  F\nEd                      62  M\n",
            ),
        ],
    );
    // the running total of the manual differs: see docs/differences.md
}

#[test]
#[ignore = "needs PSPP"]
fn reread_by_kind_and_by_column() {
    compare(
        "reread",
        [removed]
        &[],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn leave_outside_an_input_program() {
    compare(
        "leave",
        "DATA LIST LIST /Var1 Var2 Var3.\n\
         BEGIN DATA\n\
         1 2 3\n\
         4 5 6\n\
         7 8 9\n\
         END DATA.\n\
         COMPUTE TotalVar1=TotalVar1+Var1.\n\
         LEAVE TotalVar1.\n\
         IF (Var1 = 4) other = 10.\n\
         LEAVE other.",
        &[],
    );
    compare(
        "leave-lag",
        "DATA LIST LIST /DEPT SALARY.\n\
         BEGIN DATA\n\
         2 100\n\
         1 50\n\
         2 30\n\
         1 20\n\
         1 5\n\
         END DATA.\n\
         SORT CASES DEPT.\n\
         IF DEPT NE LAG(DEPT,1) TSALARY=0.\n\
         COMPUTE TSALARY=TSALARY+SALARY.\n\
         LEAVE TSALARY.\n\
         FORMATS TSALARY (DOLLAR8)/ SALARY (DOLLAR7).",
        &[],
    );
}
