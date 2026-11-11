//! PRINT, WRITE and XSAVE compared with GNU PSPP, run as a black box on the same syntax: the
//! files both programs write must be the same. Ignored by default because they need the PSPP
//! program:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_output -- --ignored
//! ```

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

const DATA: &str = "\
DATA LIST LIST /id (F3) name (A8) score (F8.2) big (F10) when (F12).
BEGIN DATA
1 'Ann' 12.5 1234567 13700000000
2 'Bob' -3.25 -42 13800000000
3 'Christopher' . 0 .
4 '' 1000.125 999999999 12000000000
END DATA.
FORMATS when (DATE11).
";

fn folder(case: &str, who: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "statscraft-reference-output-{}-{case}-{who}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `commands` after the data in both programs, each in a folder of its own, and compares
/// the files named in `files`: text files byte for byte (line ends aside), data files by
/// names and values.
fn compare(case: &str, commands: &str, files: &[&str]) {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return;
    };
    let job = format!("{DATA}{commands}\nEXECUTE.\n");
    let theirs = folder(case, "pspp");
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

    let ours = folder(case, "ours");
    let mut session = Session::default();
    session
        .run(&format!("CD '{}'.\n{job}", ours.display()), None)
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));

    for file in files {
        let (a, b) = (theirs.join(file), ours.join(file));
        if file.ends_with(".sav") {
            same_data(case, &a, &b);
        } else {
            let read = |p: &Path| fs::read_to_string(p).unwrap().replace("\r\n", "\n");
            assert_eq!(read(&b), read(&a), "{case}: {file} differs from PSPP");
        }
    }
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
fn print_to_a_file_with_carriage_control() {
    compare(
        "print",
        "PRINT OUTFILE='p.txt' /id name score big when.\n\
         PRINT EJECT OUTFILE='p.txt' /'Case' id.\n\
         PRINT SPACE OUTFILE='p.txt' 2.\n\
         PRINT OUTFILE='p.txt' /id /name score.",
        &["p.txt"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn write_in_dictionary_and_given_formats() {
    compare(
        "write",
        "WRITE OUTFILE='w.txt' /id name score big when.\n\
         WRITE OUTFILE='w.txt' /score (COMMA10.2) big (DOLLAR14) score (PCT8.1) when (ADATE10).\n\
         WRITE OUTFILE='w.txt' /score (E10.3) id (N5) name (A8) when (SDATE10) big (F4).",
        &["w.txt"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn write_in_columns_and_format_lists() {
    compare(
        "columns",
        "WRITE OUTFILE='c.txt' /'#' 1 id 3-5 name 7-14 (A) score 16-23 (2) 'end' 25.\n\
         WRITE OUTFILE='c.txt' /id score (T5,F4,2X,F8.1) big (F12) 'x' / score big (2(F12.1,1X)).\n\
         WRITE OUTFILE='c.txt' /id 1-2 big 5-14 (COMMA).",
        &["c.txt"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn print_and_write_formats_change_the_output() {
    compare(
        "formats",
        "PRINT FORMATS score (F6.1) / big (COMMA14).\n\
         WRITE FORMATS score (F10.4) / id (F2).\n\
         PRINT OUTFILE='p.txt' /id score big.\n\
         WRITE OUTFILE='w.txt' /id score big.",
        &["p.txt", "w.txt"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn headings_and_blank_lines_by_case() {
    compare(
        "headings",
        "DO IF $CASENUM = 1.\n\
         PRINT OUTFILE='h.txt' /'Name' 1 'Score' 12.\n\
         END IF.\n\
         COMPUTE #sum = #sum + id.\n\
         PRINT OUTFILE='h.txt' /name 1-8 (A) score 10-17 (2) #sum (F5).\n\
         PRINT SPACE OUTFILE='h.txt' id - 2.",
        &["h.txt"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn xsave_writes_the_cases_it_sees() {
    compare(
        "xsave",
        "COMPUTE #t = id * 10.\n\
         COMPUTE double = #t / 5.\n\
         XSAVE OUTFILE='a.sav' /DROP=big /RENAME=(name=who).\n\
         XSAVE OUTFILE='b.sav' /KEEP=double id.\n\
         SELECT IF id < 3.\n\
         COMPUTE later = 1.\n\
         XSAVE OUTFILE='c.sav'.",
        &["a.sav", "b.sav", "c.sav"],
    );
}
