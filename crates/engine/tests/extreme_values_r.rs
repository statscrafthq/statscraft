//! Extreme values in files: the largest and smallest doubles, denormals, minus zero, the numbers
//! beside system-missing, LOWEST and HIGHEST in missing ranges, and very old and far dates,
//! written as .sav, .zsav, .por, CSV and workbook files and read back by the engine.
//!
//! Set `DUMP_DIR` to a folder to write the files there instead of comparing, so that R (haven,
//! readxl and the base readers) can read them and the bits it finds can be kept below. The kept
//! bits are a snapshot of what R was shown when the tests were written: the test itself runs
//! neither, so it tells that a file changed or reads differently.

use statscraft_data::{Dataset, MissingValues, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

/// The values, by their bits: the name and the double.
fn values() -> Vec<(&'static str, f64)> {
    let bits = f64::from_bits;
    vec![
        ("max", bits(0x7FEF_FFFF_FFFF_FFFF)),
        ("lowest", bits(0xFFEF_FFFF_FFFF_FFFE)),
        ("near_sysmis", bits(0xFFEF_FFFF_FFFF_FFFD)),
        ("min_normal", bits(0x0010_0000_0000_0000)),
        ("denormal_min", bits(0x0000_0000_0000_0001)),
        ("denormal_max", bits(0x000F_FFFF_FFFF_FFFF)),
        ("neg_denormal", bits(0x8000_0000_0000_0001)),
        ("neg_zero", bits(0x8000_0000_0000_0000)),
        ("zero", 0.0),
        ("third", 1.0 / 3.0),
        ("half_max_neg", bits(0xFFE0_0000_0000_0000)),
        ("two53_plus_2", bits(0x4340_0000_0000_0001)),
        ("big_pi", 3.141_592_653_589_793e300),
        ("tiny_pi", 3.141_592_653_589_793e-300),
        ("one_ulp_below_one", bits(0x3FEF_FFFF_FFFF_FFFF)),
        // the most negative double is system-missing in a .sav file
        ("sysmis", bits(0xFFEF_FFFF_FFFF_FFFF)),
    ]
}

/// Days from 1970-01-01 to a civil date (proleptic Gregorian, as R counts them).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds from 1582-10-14 to midnight of a civil date.
fn seconds(y: i64, m: i64, d: i64) -> f64 {
    (days(y, m, d) * 86_400 + 12_219_379_200) as f64
}

/// The dates: a label and the year, month and day.
const DATES: &[(&str, i64, i64, i64)] = &[
    ("year_1", 1, 1, 1),
    ("year_1000", 1000, 6, 15),
    ("before_the_calendar", 1582, 10, 4),
    ("calendar_start", 1582, 10, 14),
    ("after_start", 1582, 10, 15),
    ("year_1899", 1899, 12, 30),
    ("year_1900", 1900, 1, 1),
    ("before_march_1900", 1900, 2, 28),
    ("march_1900", 1900, 3, 1),
    ("epoch_eve", 1969, 12, 31),
    ("epoch", 1970, 1, 1),
    ("leap_day", 2000, 2, 29),
    ("y2038", 2038, 1, 19),
    ("y2262", 2262, 4, 12),
    ("year_9999", 9999, 12, 31),
];

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-extreme-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The text of the dataset of values and the one of dates.
fn data_list() -> String {
    let rows: String = values().iter().map(|(_, v)| format!("{v:e}\n")).collect();
    let dates: String = DATES
        .iter()
        .map(|&(_, y, m, d)| {
            let day = seconds(y, m, d);
            // the same day at the last second
            format!("{day:e} {:e}\n", day + 86_399.0)
        })
        .collect();
    format!(
        "DATA LIST FREE / x (F40).\nBEGIN DATA\n{rows}END DATA.\n\
         SAVE OUTFILE='vals.sav'.\nSAVE OUTFILE='vals.zsav' /ZCOMPRESSED.\nEXPORT OUTFILE='vals.por'.\n\
         SAVE TRANSLATE OUTFILE='vals.csv' /TYPE=CSV /FIELDNAMES /ENCODING='UTF8' BOM=NO.\n\
         SAVE TRANSLATE OUTFILE='vals.xlsx' /TYPE=XLSX /FIELDNAMES.\n\
         NEW FILE.\n\
         DATA LIST FREE / d (F40) t (F40).\nBEGIN DATA\n{dates}END DATA.\n\
         FORMATS d (DATE11) t (DATETIME20).\n\
         SAVE OUTFILE='dates.sav'.\nSAVE OUTFILE='dates.zsav' /ZCOMPRESSED.\nEXPORT OUTFILE='dates.por'.\n\
         SAVE TRANSLATE OUTFILE='dates.csv' /TYPE=CSV /FIELDNAMES /ENCODING='UTF8' BOM=NO.\n\
         SAVE TRANSLATE OUTFILE='dates.xlsx' /TYPE=XLSX /FIELDNAMES.\n\
         NEW FILE.\n\
         DATA LIST LIST / a b c d e (F8.2).\nBEGIN DATA\n1 2 3 4 5\n-5 150 -1 50 99\n0 100 99 7 7\nEND DATA.\n\
         MISSING VALUES a (LOWEST THRU 0) b (100 THRU HIGHEST) c (LOWEST THRU -1, 99) d (50 THRU 60) e (99).\n\
         SAVE OUTFILE='ranges.sav'.\nSAVE OUTFILE='ranges.zsav' /ZCOMPRESSED.\nEXPORT OUTFILE='ranges.por'.\n"
    )
}

/// The bits R (haven, readxl and the base readers) found in each file the engine wrote, one
/// entry for each value or date: `NA` where R finds a missing value.
const R_BITS: &[(&str, &[&str])] = &[
    (
        "vals sav x",
        &[
            "NA",
            "NA",
            "FFEFFFFFFFFFFFFD",
            "0010000000000000",
            "0000000000000001",
            "000FFFFFFFFFFFFF",
            "8000000000000001",
            "8000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB14B6BBF",
            "3FEFFFFFFFFFFFFF",
            "NA",
        ],
    ),
    (
        "vals zsav x",
        &[
            "NA",
            "NA",
            "FFEFFFFFFFFFFFFD",
            "0010000000000000",
            "0000000000000001",
            "000FFFFFFFFFFFFF",
            "8000000000000001",
            "8000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB14B6BBF",
            "3FEFFFFFFFFFFFFF",
            "NA",
        ],
    ),
    (
        "vals por x",
        &[
            "7FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFD",
            "FFEFFFFFFFFFFFFE",
            "00186766AD4D3686",
            "0000000000000002",
            "00186766AD4D3684",
            "8000000000000002",
            "0000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB145681E",
            "3FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFF",
        ],
    ),
    (
        "vals csv x",
        &[
            "7FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFE",
            "FFEFFFFFFFFFFFFD",
            "0010000000000000",
            "0000000000000001",
            "000FFFFFFFFFFFFF",
            "8000000000000001",
            "8000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB14B6BBF",
            "3FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFF",
        ],
    ),
    (
        "vals xlsx_text x",
        &[
            "7FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFE",
            "FFEFFFFFFFFFFFFD",
            "0010000000000000",
            "0000000000000001",
            "000FFFFFFFFFFFFF",
            "8000000000000001",
            "8000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB14B6BBF",
            "3FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFF",
        ],
    ),
    (
        "vals xlsx x",
        &[
            "7FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFE",
            "FFEFFFFFFFFFFFFD",
            "0010000000000000",
            "0000000000000001",
            "000FFFFFFFFFFFFF",
            "8000000000000001",
            "8000000000000000",
            "0000000000000000",
            "3FD5555555555555",
            "FFE0000000000000",
            "4340000000000001",
            "7E52C3AE4DD16CAF",
            "01C0D4CAB14B6BBF",
            "3FEFFFFFFFFFFFFF",
            "FFEFFFFFFFFFFFFF",
        ],
    ),
    (
        "dates sav d",
        &[
            "C125F27400000000",
            "C1159D2000000000",
            "C10143F000000000",
            "C10143A000000000",
            "C101439800000000",
            "C0D8F84000000000",
            "C0D8F7C000000000",
            "C0D8E94000000000",
            "C0D8E90000000000",
            "BFF0000000000000",
            "0000000000000000",
            "40C5840000000000",
            "40D845C000000000",
            "40FA100000000000",
            "4146605000000000",
        ],
    ),
    (
        "dates sav t",
        &[
            "C22CEF214B020000",
            "C21C7EA06A040000",
            "C206C3065C080000",
            "C206C29CE4080000",
            "C206C29258080000",
            "C1E0757A00200000",
            "C1E07525A0200000",
            "C1E06B96C0200000",
            "C1E06B6C90200000",
            "BFF0000000000000",
            "40F517F000000000",
            "41CC5E2EBF800000",
            "41E000247FE00000",
            "42012E168BF80000",
            "424D7FFA20BF8000",
        ],
    ),
    (
        "dates zsav d",
        &[
            "C125F27400000000",
            "C1159D2000000000",
            "C10143F000000000",
            "C10143A000000000",
            "C101439800000000",
            "C0D8F84000000000",
            "C0D8F7C000000000",
            "C0D8E94000000000",
            "C0D8E90000000000",
            "BFF0000000000000",
            "0000000000000000",
            "40C5840000000000",
            "40D845C000000000",
            "40FA100000000000",
            "4146605000000000",
        ],
    ),
    (
        "dates zsav t",
        &[
            "C22CEF214B020000",
            "C21C7EA06A040000",
            "C206C3065C080000",
            "C206C29CE4080000",
            "C206C29258080000",
            "C1E0757A00200000",
            "C1E07525A0200000",
            "C1E06B96C0200000",
            "C1E06B6C90200000",
            "BFF0000000000000",
            "40F517F000000000",
            "41CC5E2EBF800000",
            "41E000247FE00000",
            "42012E168BF80000",
            "424D7FFA20BF8000",
        ],
    ),
    (
        "dates por d",
        &[
            "C125F27400000000",
            "C1159D2000000000",
            "C10143F000000000",
            "C10143A000000000",
            "C101439800000000",
            "C0D8F84000000000",
            "C0D8F7C000000000",
            "C0D8E94000000000",
            "C0D8E90000000000",
            "BFF0000000000000",
            "0000000000000000",
            "40C5840000000000",
            "40D845C000000000",
            "40FA100000000000",
            "4146605000000000",
        ],
    ),
    (
        "dates por t",
        &[
            "C22CEF214B020000",
            "C21C7EA06A040000",
            "C206C3065C080000",
            "C206C29CE4080000",
            "C206C29258080000",
            "C1E0757A00200000",
            "C1E07525A0200000",
            "C1E06B96C0200000",
            "C1E06B6C90200000",
            "BFF0000000000000",
            "40F517F000000000",
            "41CC5E2EBF800000",
            "41E000247FE00000",
            "42012E168BF80000",
            "424D7FFA20BF8000",
        ],
    ),
    (
        "dates csv d",
        &[
            "C125F27400000000",
            "C1159D2000000000",
            "C10143F000000000",
            "C10143A000000000",
            "C101439800000000",
            "C0D8F84000000000",
            "C0D8F7C000000000",
            "C0D8E94000000000",
            "C0D8E90000000000",
            "BFF0000000000000",
            "0000000000000000",
            "40C5840000000000",
            "40D845C000000000",
            "40FA100000000000",
            "4146605000000000",
        ],
    ),
    (
        "dates csv t",
        &[
            "C22CEF214B020000",
            "C21C7EA06A040000",
            "C206C3065C080000",
            "C206C29CE4080000",
            "C206C29258080000",
            "C1E0757A00200000",
            "C1E07525A0200000",
            "C1E06B96C0200000",
            "C1E06B6C90200000",
            "BFF0000000000000",
            "40F517F000000000",
            "41CC5E2EBF800000",
            "41E000247FE00000",
            "42012E168BF80000",
            "424D7FFA20BF8000",
        ],
    ),
    (
        "dates xlsx d",
        &[
            "C125F27400000000",
            "C1159D2000000000",
            "C10143F000000000",
            "C10143A000000000",
            "C101439800000000",
            "C0D8F84000000000",
            "C0D8F7C000000000",
            "C0D8E94000000000",
            "C0D8E90000000000",
            "BFF0000000000000",
            "0000000000000000",
            "40C5840000000000",
            "40D845C000000000",
            "40FA100000000000",
            "4146605000000000",
        ],
    ),
    (
        "dates xlsx t",
        &[
            "C22CEF214B020000",
            "C21C7EA06A040000",
            "C206C3065C080000",
            "C206C29CE4080000",
            "C206C29258080000",
            "C1E0757A00200000",
            "C1E07525A0200000",
            "C1E06B96C0200000",
            "C1E06B6C90200000",
            "BFF0000000000000",
            "40F517F000000000",
            "41CC5E2EBF800000",
            "41E000247FE00000",
            "42012E168BF80000",
            "424D7FFA20BF8000",
        ],
    ),
];

fn written(dir: &Path) -> Session {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!("CD '{}'.\n{}", dir.display(), data_list());
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

#[test]
fn dump_the_files_for_r() {
    let Some(dump) = std::env::var_os("DUMP_DIR") else {
        return;
    };
    let dir = temp_dir("dump");
    written(&dir);
    std::fs::create_dir_all(&dump).unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), Path::new(&dump).join(entry.file_name())).unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
}

fn bits_of(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v
            .iter()
            .map(|v| v.map_or("NA".to_string(), |v| format!("{:016X}", v.to_bits())))
            .collect(),
        Values::String(_) => panic!("{name} is text"),
    }
}

fn hex(value: f64) -> String {
    format!("{:016X}", value.to_bits())
}

/// The values as the engine reads each kind of file back: all exactly, except that the most
/// negative double is system-missing in a .sav file and minus zero is plain zero in a portable
/// file (which has no sign of a zero).
fn engine_reads(kind: &str) -> Vec<String> {
    values()
        .into_iter()
        .map(|(name, v)| match (kind, name) {
            ("sav" | "zsav", "sysmis") => "NA".to_string(),
            ("por", "neg_zero") => hex(0.0),
            _ => hex(v),
        })
        .collect()
}

#[test]
fn the_engine_reads_back_the_extreme_values_it_wrote() {
    let dir = temp_dir("back");
    let mut session = written(&dir);
    for (file, kind) in [("vals.sav", "sav"), ("vals.zsav", "zsav")] {
        let data = Dataset::read_sav(dir.join(file)).unwrap();
        assert_eq!(bits_of(&data, "x"), engine_reads(kind), "{file}");
    }
    let data = Dataset::read_por(dir.join("vals.por")).unwrap();
    assert_eq!(bits_of(&data, "x"), engine_reads("por"), "vals.por");
    session
        .run(
            "GET DATA /TYPE=TXT /FILE='vals.csv' /FIRSTCASE=2 /VARIABLES=x F40.",
            None,
        )
        .unwrap();
    assert_eq!(
        bits_of(session.dataset().unwrap(), "x"),
        engine_reads("csv")
    );
    session
        .run("GET DATA /TYPE=XLSX /FILE='vals.xlsx'.", None)
        .unwrap();
    assert_eq!(
        bits_of(session.dataset().unwrap(), "x"),
        engine_reads("xlsx")
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn r_bits(name: &str) -> &'static [&'static str] {
    R_BITS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no bits kept for {name}"))
        .1
}

/// R's reading of the value files is what the files hold: a .sav or .zsav reader takes the
/// highest and lowest doubles and the system-missing one for missing values, the portable file
/// reader of haven works the digits out with a small error (and badly for the smallest numbers),
/// text and workbook readers are exact.
#[test]
fn r_reads_the_value_files_as_the_engine_means_them() {
    let wanted: Vec<f64> = values().into_iter().map(|(_, v)| v).collect();
    for kind in ["sav", "zsav"] {
        let got = r_bits(&format!("vals {kind} x"));
        for (k, (name, v)) in values().into_iter().enumerate() {
            let reserved = ["max", "lowest", "sysmis"].contains(&name);
            let want = if reserved { "NA".to_string() } else { hex(v) };
            assert_eq!(got[k], want, "{kind} {name}");
        }
    }
    for name in ["vals csv x", "vals xlsx_text x", "vals xlsx x"] {
        let got = r_bits(name);
        for (k, v) in wanted.iter().enumerate() {
            assert_eq!(got[k], hex(*v), "{name} row {k}");
        }
    }
    // haven works the digits of a portable file out by powers of 30, which is exact to the last
    // places for ordinary sizes, to nine digits near 1e-300, and not at all for the denormals and
    // the smallest normal numbers
    let por = r_bits("vals por x");
    for (k, v) in wanted.iter().enumerate() {
        let read = f64::from_bits(u64::from_str_radix(por[k], 16).unwrap());
        if v.abs() < 1e-300 && *v != 0.0 {
            continue;
        }
        let close = *v == read || ((read - v) / v).abs() < 1e-9;
        assert!(close, "por row {k}: {v:e} read as {read:e}");
    }
}

#[test]
fn dates_from_year_1_to_9999_keep_their_seconds_and_their_text() {
    let dir = temp_dir("dates");
    let mut session = written(&dir);
    let mut seconds_wanted = Vec::new();
    let mut days_wanted = Vec::new();
    for &(_, y, m, d) in DATES {
        seconds_wanted.push(seconds(y, m, d));
        days_wanted.push(days(y, m, d) as f64);
    }
    // the engine reads back the seconds it wrote
    for file in ["dates.sav", "dates.zsav"] {
        let data = Dataset::read_sav(dir.join(file)).unwrap();
        let want: Vec<String> = seconds_wanted.iter().map(|s| hex(*s)).collect();
        assert_eq!(bits_of(&data, "d"), want, "{file}");
        let at_the_end: Vec<String> = seconds_wanted.iter().map(|s| hex(s + 86_399.0)).collect();
        assert_eq!(bits_of(&data, "t"), at_the_end, "{file}");
    }
    let data = Dataset::read_por(dir.join("dates.por")).unwrap();
    let want: Vec<String> = seconds_wanted.iter().map(|s| hex(*s)).collect();
    assert_eq!(bits_of(&data, "d"), want, "por");
    // R reads every one of them as the date it is: the days since 1970 of the civil date
    for name in [
        "dates sav d",
        "dates zsav d",
        "dates por d",
        "dates csv d",
        "dates xlsx d",
    ] {
        let want: Vec<String> = days_wanted.iter().map(|d| hex(*d)).collect();
        assert_eq!(r_bits(name), want, "{name}");
    }
    for name in [
        "dates sav t",
        "dates zsav t",
        "dates por t",
        "dates csv t",
        "dates xlsx t",
    ] {
        let want: Vec<String> = days_wanted
            .iter()
            .map(|d| hex(d * 86_400.0 + 86_399.0))
            .collect();
        assert_eq!(r_bits(name), want, "{name}");
    }
    // the text of the CSV file is the date as month, day and year, with the year in four digits
    let csv = std::fs::read_to_string(dir.join("dates.csv")).unwrap();
    let first: Vec<&str> = csv.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(first, ["01/01/0001", "01/01/0001 23:59:59"]);
    let last: Vec<&str> = csv.lines().last().unwrap().split(',').collect();
    assert_eq!(last, ["12/31/9999", "12/31/9999 23:59:59"]);
    // a workbook cell Excel can show is a date; one it cannot (before March 1900) holds the text
    // of the date, never asterisks
    session
        .run("GET DATA /TYPE=XLSX /FILE='dates.xlsx'.", None)
        .unwrap();
    let data = session.dataset().unwrap();
    let Values::String(text) = data.variable("d").unwrap().values() else {
        // the whole column came as numbers: nothing was written as text
        panic!("the dates before 1900 are text");
    };
    let cells: Vec<&str> = text.iter().collect();
    assert_eq!(cells[0], "01/01/0001");
    assert_eq!(cells[3], "14-OCT-1582");
    assert!(cells.iter().all(|c| !c.contains('*')), "{cells:?}");
    std::fs::remove_dir_all(&dir).ok();
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extreme")
        .join(name)
}

/// Files that haven wrote: the values and dates are read to the bit, also the numbers that the
/// file holds beside system-missing, and the last value, which R held as NA, is missing.
#[test]
fn files_haven_wrote_with_extreme_values_are_read_to_the_bit() {
    let mut intended: Vec<String> = values()
        .into_iter()
        .filter(|(name, _)| *name != "sysmis")
        // haven writes minus zero as zero
        .map(|(name, v)| if name == "neg_zero" { hex(0.0) } else { hex(v) })
        .collect();
    intended.push("NA".to_string());
    for file in ["haven_vals.sav", "haven_vals.zsav"] {
        let data = Dataset::read_sav(fixture(file)).unwrap();
        assert_eq!(bits_of(&data, "x"), intended, "{file}");
    }
    let data = Dataset::read_sav(fixture("haven_dates.sav")).unwrap();
    let wanted = |extra: f64| -> Vec<String> {
        DATES
            .iter()
            .map(|&(_, y, m, d)| hex(seconds(y, m, d) + extra))
            .collect()
    };
    assert_eq!(bits_of(&data, "d"), wanted(0.0));
    assert_eq!(bits_of(&data, "t"), wanted(86_399.0));
    assert_eq!(data.variable("d").unwrap().format(), "DATE11");
    assert_eq!(data.variable("t").unwrap().format(), "DATETIME20");
}

/// LOWEST and HIGHEST in the missing ranges: the file holds the second most negative and the
/// most positive double (the most negative one is system-missing), which haven reads as minus
/// and plus infinity, and the engine reads back as the ends of the range it was given.
#[test]
fn lowest_and_highest_in_missing_ranges_are_what_other_readers_take_them_for() {
    let dir = temp_dir("ranges");
    written(&dir);
    let low = f64::MIN;
    let high = f64::MAX;
    let wanted = [
        (
            "a",
            MissingValues::Range {
                low,
                high: 0.0,
                extra: None,
            },
        ),
        (
            "b",
            MissingValues::Range {
                low: 100.0,
                high,
                extra: None,
            },
        ),
        (
            "c",
            MissingValues::Range {
                low,
                high: -1.0,
                extra: Some(99.0),
            },
        ),
        (
            "d",
            MissingValues::Range {
                low: 50.0,
                high: 60.0,
                extra: None,
            },
        ),
        ("e", MissingValues::Numbers(vec![99.0])),
    ];
    for file in ["ranges.sav", "ranges.zsav"] {
        let data = Dataset::read_sav(dir.join(file)).unwrap();
        for (name, want) in &wanted {
            let got = data.variable(name).unwrap().missing_values().clone();
            assert_eq!(&got, want, "{file} {name}");
        }
    }
    let data = Dataset::read_por(dir.join("ranges.por")).unwrap();
    for (name, want) in &wanted {
        let got = data.variable(name).unwrap().missing_values().clone();
        assert_eq!(&got, want, "ranges.por {name}");
    }
    // the bounds in the file itself: LOWEST is 0xFFEFFFFFFFFFFFFE, HIGHEST 0x7FEFFFFFFFFFFFFF
    let file = std::fs::read(dir.join("ranges.sav")).unwrap();
    let word = |at: usize| u64::from_le_bytes(file[at..at + 8].try_into().unwrap());
    let int = |at: usize| i32::from_le_bytes(file[at..at + 4].try_into().unwrap());
    let mut at = 176;
    let mut bounds = Vec::new();
    for _ in 0..5 {
        assert_eq!(int(at), 2, "a variable record");
        let count = int(at + 12);
        let n = count.unsigned_abs() as usize;
        bounds.push((0..n).map(|k| word(at + 32 + 8 * k)).collect::<Vec<_>>());
        at += 32 + 8 * n;
    }
    assert_eq!(bounds[0], [0xFFEF_FFFF_FFFF_FFFE, 0.0f64.to_bits()]);
    assert_eq!(bounds[1], [100.0f64.to_bits(), 0x7FEF_FFFF_FFFF_FFFF]);
    assert_eq!(bounds[2][0], 0xFFEF_FFFF_FFFF_FFFE);
    std::fs::remove_dir_all(&dir).ok();
}
