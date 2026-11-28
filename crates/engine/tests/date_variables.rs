//! Date, date-time and time-of-day variables hold seconds, and a procedure sees those seconds:
//! each command below gives the same numbers on a variable of date format as on a copy of it with
//! a plain number format (the text cells differ by the format and are not compared). A file
//! round trip keeps the values of such variables in .sav, Stata and workbook files. A text
//! file writes dates as mm/dd/yyyy and times as hh:mm:ss unless FORMAT=VARIABLE is asked for
//! (the manual), and Stata and workbook files have no European date format, so those come back
//! in another format with the same value. Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "\
DATA LIST FIXED /g 1 d 3-12 (EDATE) dt 14-33 (DATETIME) t 35-42 (TIME) x 44-47 (2) y 49-53 (2) b 55.
BEGIN DATA
1 29-05-2019 29-MAY-2019 12:57:00 12:57:00 5.14 4.41 0
2 16-07-2019 16-JUL-2019 12:06:01 12:06:01 8.06 5.36 0
3 12-01-2021 12-JAN-2021 15:05:34 15:05:34 1.06 9.7 1
1 07-01-2021 07-JAN-2021 05:12:09 05:12:09 8.34 6.63 0
2 18-03-2021 18-MAR-2021 04:19:26 04:19:26 6.96 10.95 1
3 13-06-2021 13-JUN-2021 17:37:13 17:37:13 1.6 13.33 1
1 04-12-2019 04-DEC-2019 03:27:28 03:27:28 7.1 5.07 0
2 19-07-2021 19-JUL-2021 12:07:32 12:07:32 7.24 12.18 1
3 27-06-2021 27-JUN-2021 22:18:01 22:18:01 6.84 12.94 1
1 13-11-2019 13-NOV-2019 21:12:26 21:12:26 8.88 8.37 1
2 03-10-2019 03-OCT-2019 14:15:27 14:15:27 3.07 5.77 0
3 13-04-2019 13-APR-2019 04:22:04 04:22:04 4.66 5.79 0
1 12-03-2019 12-MAR-2019 15:27:07 15:27:07 1.83 2.73 0
2 13-04-2019 13-APR-2019 21:43:17 21:43:17 5.41 6.64 0
3 03-10-2020 03-OCT-2020 13:18:41 13:18:41 8.51 8.9 1
1 08-09-2019 08-SEP-2019 18:05:39 18:05:39 5.41 6.18 0
2 24-11-2019 24-NOV-2019 13:06:16 13:06:16 1.97 7.64 1
3 15-08-2019 15-AUG-2019 13:49:44 13:49:44 2.59 6.38 0
1 25-06-2020 25-JUN-2020 21:13:26 21:13:26 7.53 9.29 1
2 21-04-2020 21-APR-2020 04:48:33 04:48:33 7.34 9.23 1
3 20-10-2020 20-OCT-2020 13:50:07 13:50:07 4.65 12.81 1
1 30-01-2021 30-JAN-2021 21:42:30 21:42:30 4.82 12.13 1
2 02-08-2021 02-AUG-2021 12:13:33 12:13:33 6.37 10.58 1
3 13-12-2019 13-DEC-2019 11:05:28 11:05:28 3.93 9.76 1
1 26-07-2020 26-JUL-2020 03:02:27 03:02:27 7.66 11.28 1
2 06-04-2019 06-APR-2019 04:37:57 04:37:57 1.35 3.73 0
3 15-07-2020 15-JUL-2020 00:12:19 00:12:19 8.52 7.59 1
1 09-03-2019 09-MAR-2019 02:56:33 02:56:33 5.66 2.41 0
2 24-07-2019 24-JUL-2019 00:15:06 00:15:06 6.03 5.66 0
3 14-07-2020 14-JUL-2020 13:46:31 13:46:31 2.97 11.07 1
1 19-05-2021 19-MAY-2021 22:00:49 22:00:49 4.98 14.25 1
2 18-10-2019 18-OCT-2019 14:51:36 14:51:36 4.16 7.12 1
3 18-03-2019 18-MAR-2019 08:09:35 08:09:35 8.67 2.65 0
1 28-04-2021 28-APR-2021 08:05:43 08:05:43 4.52 11.99 1
2 09-10-2020 09-OCT-2020 23:22:23 23:22:23 4.4 12.84 1
3 02-08-2020 02-AUG-2020 11:26:02 11:26:02 4.28 10.3 1
1 30-05-2020 30-MAY-2020 21:11:39 21:11:39 3.85 7.14 1
2 11-02-2021 11-FEB-2021 23:26:37 23:26:37 6.45 12.21 1
3 14-08-2019 14-AUG-2019 15:09:37 15:09:37 5.97 6.85 0
1 01-06-2019 01-JUN-2019 07:25:35 07:25:35 8.77 2.99 0
END DATA.
";
const COMMANDS: &[&str] = &[
    "DESCRIPTIVES d dt t /STATISTICS=MEAN STDDEV MIN MAX.",
    "FREQUENCIES d dt t /STATISTICS=MEAN MEDIAN MIN MAX /FORMAT=NOTABLE.",
    "FREQUENCIES g /NTILES=4.",
    "MEANS d dt BY g /CELLS=MEAN COUNT STDDEV.",
    "EXAMINE d BY g /PLOT=NONE.",
    "T-TEST GROUPS=b(0,1) /VARIABLES=d dt t.",
    "T-TEST PAIRS=d WITH dt.",
    "ONEWAY d dt BY g.",
    "CORRELATIONS d dt t y.",
    "NONPAR CORR d dt y.",
    "REGRESSION /DEPENDENT y /METHOD=ENTER d t.",
    "GLM y BY g WITH d.",
    "NPAR TESTS /K-W=d BY g(1,3).",
    "NPAR TESTS /M-W=dt BY b(0,1).",
    "NPAR TESTS /WILCOXON=d WITH dt.",
    "CROSSTABS g BY b.",
    "AGGREGATE /OUTFILE=* /BREAK=g /dmin=MIN(d) /dmax=MAX(d) /dmean=MEAN(dt) /n=N.",
    "RANK VARIABLES=d dt.",
    "FACTOR /VARIABLES d dt t y.",
    "CLUSTER d dt /METHOD=WARD.",
    "COXREG d /STATUS=b(1) /METHOD=ENTER y.",
    "KM d BY g /STATUS=b(1).",
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=d t y.",
    "SUMMARIZE /TABLES=d dt t BY g /CELLS=COUNT MEAN.",
    "CTABLES /TABLE d BY g.",
];

const COPY: &str = "COMPUTE dn = d.
COMPUTE dtn = dt.
COMPUTE tn = t.
FORMATS dn dtn tn (F20.0).
";

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?} in {source}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error} in {source}", command.first_line),
        }
    }
    tables
}

/// The command with each of d, dt and t as its plain copy dn, dtn and tn.
fn on_the_copies(command: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let is_name = matches!(word.as_str(), "d" | "dt" | "t") && !out.ends_with('/');
        out.push_str(word);
        if is_name {
            out.push('n');
        }
        word.clear();
    };
    for c in command.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

fn numbers_agree(a: &[Table], b: &[Table], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: number of tables");
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.rows.len(), y.rows.len(), "{what}: {}", x.title);
        for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
            for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                if let (Cell::Number(p), Cell::Number(q)) = (ca, cb) {
                    assert!(
                        (p - q).abs() <= 1e-7 * p.abs().max(q.abs()).max(1e-3),
                        "{what}: {} row {r} column {c}: {p} is not {q}",
                        x.title
                    );
                }
            }
        }
    }
}

#[test]
fn a_procedure_sees_the_seconds_of_a_date_variable() {
    for command in COMMANDS {
        let dates = run(&format!("{DATA}{command}\n"));
        let plain = run(&format!("{DATA}{COPY}{}\n", on_the_copies(command)));
        numbers_agree(&dates, &plain, command);
    }
}

fn keeps_the_values(write: &str, read: &str, name: &str) {
    let dir = std::env::temp_dir().join(format!("statscraft-dates-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("dates.{name}")).display().to_string();
    let show = "FORMATS d dt t (F20.0).\nLIST d dt t.\n";
    let before = run(&format!("{DATA}{show}"));
    let after = run(&format!(
        "{DATA}{}\nNEW FILE.\n{}\n{show}",
        write.replace("@FILE@", &format!("'{path}'")),
        read.replace("@FILE@", &format!("'{path}'")),
    ));
    numbers_agree(&before, &after, name);
    assert!(
        before[0]
            .rows
            .iter()
            .zip(&after[0].rows)
            .all(|(a, b)| a == b),
        "{name}: the listed values differ"
    );
}

#[test]
fn date_values_survive_a_sav_file() {
    keeps_the_values("SAVE OUTFILE=@FILE@.", "GET FILE=@FILE@.", "sav");
}

#[test]
fn date_values_survive_a_stata_file() {
    keeps_the_values(
        "SAVE TRANSLATE OUTFILE=@FILE@ /TYPE=STATA /REPLACE.",
        "GET STATA FILE=@FILE@.",
        "dta",
    );
}

#[test]
fn date_values_survive_a_workbook() {
    keeps_the_values(
        "SAVE TRANSLATE OUTFILE=@FILE@ /TYPE=XLS /FIELDNAMES /REPLACE.",
        "GET DATA /TYPE=XLSX /FILE=@FILE@ /READNAMES=ON.",
        "xlsx",
    );
}
