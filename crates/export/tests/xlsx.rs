//! Workbooks written from real runs on the synthetic survey fixture, opened again and checked
//! cell by cell against the tables they came from.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table, header_rows};
use statscraft_export::{Item, write_xlsx};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Text(String),
    Number(f64),
    Blank,
}

/// One cell as the workbook stores it: value, number format code and bold font.
#[derive(Debug, Clone)]
struct XCell {
    value: Value,
    format: String,
    bold: bool,
}

#[derive(Debug)]
struct Sheet {
    name: String,
    cells: HashMap<String, XCell>,
    merges: Vec<String>,
    /// The first cell below and right of the frozen panes.
    pane: Option<String>,
    /// Column widths, A first.
    widths: Vec<f64>,
}

impl Sheet {
    fn at(&self, row: u32, column: usize) -> XCell {
        self.cells.get(&a1(row, column)).cloned().unwrap_or(XCell {
            value: Value::Blank,
            format: "General".into(),
            bold: false,
        })
    }

    fn value(&self, reference: &str) -> Value {
        self.cells
            .get(reference)
            .map_or(Value::Blank, |c| c.value.clone())
    }
}

/// "C5" for zero-based row 4, column 2.
fn a1(row: u32, column: usize) -> String {
    let mut letters = String::new();
    let mut n = column + 1;
    while n > 0 {
        letters.insert(0, (b'A' + ((n - 1) % 26) as u8) as char);
        n = (n - 1) / 26;
    }
    format!("{letters}{}", row + 1)
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    Some(&tag[start..start + tag[start..].find('"')?])
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The parts of `xml` that start with `open`, each up to the next `open`.
fn elements<'a>(xml: &'a str, open: &'a str) -> impl Iterator<Item = &'a str> {
    xml.split(open).skip(1)
}

/// Reads the sheets back with the zip crate and plain string search: enough for the XML that
/// rust_xlsxwriter writes.
fn read_xlsx(path: &PathBuf) -> Vec<Sheet> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut part = |name: &str| {
        let mut text = String::new();
        if let Ok(mut file) = zip.by_name(name) {
            file.read_to_string(&mut text).unwrap();
        }
        text
    };
    let strings: Vec<String> = elements(&part("xl/sharedStrings.xml"), "<si>")
        .map(|si| {
            let t = &si[si.find("<t").unwrap()..];
            unescape(&t[t.find('>').unwrap() + 1..t.find("</t>").unwrap()])
        })
        .collect();
    let styles = part("xl/styles.xml");
    let mut codes: HashMap<String, String> = [
        ("0", "General"),
        ("1", "0"),
        ("2", "0.00"),
        ("3", "#,##0"),
        ("4", "#,##0.00"),
    ]
    .map(|(id, code)| (id.to_string(), code.to_string()))
    .into();
    for numfmt in elements(&styles, "<numFmt ") {
        let code = unescape(attr(&format!(" {numfmt}"), "formatCode").unwrap());
        codes.insert(
            attr(&format!(" {numfmt}"), "numFmtId").unwrap().into(),
            code,
        );
    }
    let fonts = &styles[styles.find("<fonts").unwrap()..styles.find("</fonts>").unwrap()];
    let bold: Vec<bool> = elements(fonts, "<font>")
        .map(|f| f.contains("<b/>"))
        .collect();
    let xfs = &styles[styles.find("<cellXfs").unwrap()..styles.find("</cellXfs>").unwrap()];
    let xfs: Vec<(String, bool)> = elements(xfs, "<xf ")
        .map(|xf| {
            let xf = format!(" {xf}");
            let font: usize = attr(&xf, "fontId").unwrap().parse().unwrap();
            (codes[attr(&xf, "numFmtId").unwrap()].clone(), bold[font])
        })
        .collect();
    let workbook = part("xl/workbook.xml");
    let names: Vec<String> = elements(&workbook, "<sheet ")
        .map(|s| unescape(attr(&format!(" {s}"), "name").unwrap()))
        .collect();
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let xml = part(&format!("xl/worksheets/sheet{}.xml", i + 1));
            let mut cells = HashMap::new();
            for c in elements(&xml, "<c r=\"") {
                let reference = &c[..c.find('"').unwrap()];
                let tag = &c[..c.find('>').unwrap()];
                let tag = format!(" {tag}");
                let (format, bold) = attr(&tag, "s").map_or(("General".into(), false), |s| {
                    xfs[s.parse::<usize>().unwrap()].clone()
                });
                let value = match (tag.ends_with('/'), attr(&tag, "t")) {
                    (true, _) => Value::Blank,
                    (false, kind) => {
                        let v = &c[c.find("<v>").unwrap() + 3..c.find("</v>").unwrap()];
                        match kind {
                            Some("s") => Value::Text(strings[v.parse::<usize>().unwrap()].clone()),
                            None => Value::Number(v.parse().unwrap()),
                            Some(other) => panic!("cell type {other}"),
                        }
                    }
                };
                let cell = XCell {
                    value,
                    format,
                    bold,
                };
                cells.insert(reference.to_string(), cell);
            }
            let merges = elements(&xml, "<mergeCell ")
                .map(|m| attr(&format!(" {m}"), "ref").unwrap().to_string())
                .collect();
            let pane = elements(&xml, "<pane ")
                .next()
                .map(|p| attr(&format!(" {p}"), "topLeftCell").unwrap().to_string());
            let mut widths = Vec::new();
            for col in elements(&xml, "<col ") {
                let col = format!(" {col}");
                let max: usize = attr(&col, "max").unwrap().parse().unwrap();
                let width: f64 = attr(&col, "width").unwrap().parse().unwrap();
                widths.resize(max, width);
            }
            Sheet {
                name,
                cells,
                merges,
                pane,
                widths,
            }
        })
        .collect()
}

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn tables(source: &str) -> Vec<Table> {
    let source = format!("GET FILE='{}'.\n{source}", fixture("survey_basic.sav"));
    let result = Session::default().run(&source, None).unwrap();
    result.commands.into_iter().flat_map(|c| c.tables).collect()
}

/// Writes `tables` to a workbook in a folder of its own under the system temp folder, reads it
/// back and removes the folder.
fn round_trip(test: &str, tables: &[Table], one_sheet: bool) -> Vec<Sheet> {
    let dir = std::env::temp_dir().join(format!("statscraft-xlsx-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.xlsx");
    write_xlsx(&path, tables.iter().map(Item::Table), one_sheet).unwrap();
    let sheets = read_xlsx(&path);
    std::fs::remove_dir_all(&dir).unwrap();
    sheets
}

/// The Excel format each display format of our tables must get. A format missing here fails
/// the test, so a new one cannot slip through unchecked.
fn expected_format(format: &str, value: f64) -> &'static str {
    match format {
        "" if value.fract() == 0.0 => "0",
        "" => "0.00",
        "F40.0" => "0",
        "F8.3" => "0.000",
        "PCT40.1" => "0.0\"%\"",
        "COMMA12.2" => "#,##0.00",
        "COMMA10" => "#,##0",
        "DOLLAR10.2" => "\"$\"#,##0.00",
        "DOLLAR9" => "\"$\"#,##0",
        other => panic!("no expected Excel format for {other}"),
    }
}

/// Checks the title, every header level, every body cell and the caption of `table` on
/// `sheet` from row `top`, and returns the row after the caption.
fn check_table(sheet: &Sheet, table: &Table, top: u32) -> u32 {
    let title = sheet.at(top, 0);
    assert_eq!(title.value, Value::Text(table.title.clone()));
    assert!(title.bold, "the title is bold");
    let headers = header_rows(table);
    // above a row label column all header levels are one merged cell holding its labels
    for c in 0..table.row_headers {
        let label: Vec<&str> = headers
            .iter()
            .flatten()
            .filter(|cell| cell.first == c && !cell.label.is_empty())
            .map(|cell| cell.label.as_str())
            .collect();
        let expected = match label.join(" ") {
            text if text.is_empty() => Value::Blank,
            text => Value::Text(text),
        };
        let (first, last) = (top + 1, top + headers.len() as u32);
        assert_eq!(
            sheet.at(first, c).value,
            expected,
            "header {}",
            a1(first, c)
        );
        if last > first {
            let range = format!("{}:{}", a1(first, c), a1(last, c));
            assert!(sheet.merges.contains(&range), "{range} is merged");
        }
    }
    for (level, cells) in headers.iter().enumerate() {
        let row = top + 1 + level as u32;
        for cell in cells.iter().filter(|cell| cell.first >= table.row_headers) {
            let x = sheet.at(row, cell.first);
            let expected = match cell.label.as_str() {
                "" => Value::Blank,
                label => Value::Text(label.into()),
            };
            assert_eq!(x.value, expected, "header {}", a1(row, cell.first));
            assert!(x.bold, "header {} is bold", a1(row, cell.first));
            if cell.span > 1 {
                let range = format!(
                    "{}:{}",
                    a1(row, cell.first),
                    a1(row, cell.first + cell.span - 1)
                );
                assert!(sheet.merges.contains(&range), "{range} is merged");
            }
        }
    }
    let first = top + 1 + headers.len() as u32;
    for (r, cells) in table.rows.iter().enumerate() {
        let row = first + r as u32;
        for (c, cell) in cells.iter().enumerate() {
            let x = sheet.at(row, c);
            let at = a1(row, c);
            if c < table.row_headers {
                // a label repeated under the same outer labels is written once, merged down
                let label = |r: usize| -> Vec<String> {
                    (0..=c)
                        .map(|k| statscraft_engine::cell_text(table, r, k))
                        .collect()
                };
                let continues = r > 0 && label(r) == label(r - 1) && !label(r)[c].is_empty();
                if continues {
                    assert_eq!(x.value, Value::Blank, "{at} is inside a merged label");
                    continue;
                }
                let span = (r..table.rows.len())
                    .take_while(|&k| label(k) == label(r))
                    .count();
                if span > 1 && !label(r)[c].is_empty() {
                    let range = format!("{at}:{}", a1(row + span as u32 - 1, c));
                    assert!(sheet.merges.contains(&range), "{range} is merged");
                }
            }
            match cell {
                Cell::Number(n) if c >= table.row_headers => {
                    assert_eq!(x.value, Value::Number(*n), "{at}");
                    let format = table.formats.get(r).and_then(|f| f.get(c));
                    let format = format.map_or("", String::as_str);
                    assert_eq!(x.format, expected_format(format, *n), "{at} in {format}");
                }
                Cell::Empty => assert_eq!(x.value, Value::Blank, "{at}"),
                _ => assert_eq!(
                    x.value,
                    Value::Text(statscraft_engine::cell_text(table, r, c)),
                    "{at}"
                ),
            }
        }
    }
    let mut next = first + table.rows.len() as u32;
    if let Some(caption) = &table.caption {
        assert_eq!(sheet.at(next, 0).value, Value::Text(caption.clone()));
        next += 1;
    }
    next
}

#[test]
fn frequencies_keep_numbers_and_repeated_titles_get_their_own_sheet() {
    let tables = tables("FREQUENCIES gender gender.");
    let sheets = round_trip("frequencies", &tables, false);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Gender", "Gender (2)"]);
    let sheet = &sheets[0];
    check_table(sheet, &tables[0], 0);
    assert_eq!(sheet.value("A2"), Value::Text("Category".into()));
    assert_eq!(sheet.value("C3"), Value::Text("Male".into()));
    // counts 106, 88, 6 and the total 200 are numbers, not text
    for (cell, count) in [("D3", 106.0), ("D4", 88.0), ("D5", 6.0), ("D6", 200.0)] {
        assert_eq!(sheet.value(cell), Value::Number(count), "{cell}");
    }
    assert_eq!(sheet.value("E4"), Value::Number(44.0));
    // frozen below the header row and right of the row labels
    let labels = tables[0].row_headers;
    assert_eq!(sheet.pane.as_deref(), Some(a1(2, labels).as_str()));
    // the widest label, "Other or prefer to self-describe", sets the width of its column
    assert!(sheet.widths[2] > 32.0, "{:?}", sheet.widths);
    assert!(sheet.widths[3] < 15.0, "{:?}", sheet.widths);
}

#[test]
fn ctables_with_three_header_levels_layers_letters_and_small_counts() {
    let tables = tables(
        "CTABLES /HIDESMALLCOUNTS COUNT=10 \
         /TABLE region BY gender [COUNT, COLPCT.COUNT PCT40.1] BY q5_1 \
         /COMPARETEST TYPE=PROP ALPHA=0.3 ADJUST=NONE MERGE=YES.",
    );
    assert_eq!(tables.len(), 2, "one table per layer");
    let sheets = round_trip("ctables", &tables, false);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    // both layer titles are longer than 31 characters and share the first 31
    assert_eq!(
        names,
        [
            "Custom Tables (Q5. Which of the",
            "Custom Tables (Q5. Which of (2)"
        ]
    );
    for (sheet, table) in sheets.iter().zip(&tables) {
        assert_eq!(header_rows(table).len(), 3);
        check_table(sheet, table, 0);
        assert_eq!(sheet.pane.as_deref(), Some("C5"));
        for range in ["C2:K2", "C3:E3", "F3:H3", "I3:K3"] {
            assert!(sheet.merges.contains(&range.to_string()), "{range}");
        }
    }
    let sheet = &sheets[0];
    assert_eq!(sheet.value("C2"), Value::Text("Gender".into()));
    assert_eq!(sheet.value("C3"), Value::Text("Male".into()));
    assert_eq!(sheet.value("D4"), Value::Text("Column %".into()));
    // North, male: 16 cases, 8 of 39 in percent, column letter a; female count hidden as <10
    assert_eq!(sheet.value("C5"), Value::Number(16.0));
    let XCell { value, format, .. } = sheet.at(4, 3);
    let Value::Number(percent) = value else {
        panic!("{value:?}")
    };
    assert!(
        (percent - 800.0 / 39.0).abs() < 1e-9,
        "stays 20.5, not 0.205"
    );
    assert_eq!(format, "0.0\"%\"");
    assert_eq!(sheet.value("E5"), Value::Text("a".into()));
    assert_eq!(sheet.value("F5"), Value::Text("<10".into()));
    assert!(matches!(sheet.value("A10"), Value::Text(t) if t.starts_with("Two-sided z-tests")));
}

#[test]
fn crosstabs_with_chi_square() {
    let tables = tables("CROSSTABS /TABLES=gender BY region /STATISTICS=CHISQ.");
    let sheets = round_trip("crosstabs", &tables, false);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Gender by Region of residence",
            "Chi-Square Tests Gender by Regi"
        ]
    );
    for (sheet, table) in sheets.iter().zip(&tables) {
        check_table(sheet, table, 0);
    }
    let counts = &sheets[0];
    assert!(
        counts.merges.contains(&"C2:G2".to_string()),
        "region over its 5 columns"
    );
    assert_eq!(
        counts.value("C2"),
        Value::Text("Region of residence".into())
    );
    assert_eq!(counts.value("H3"), Value::Text("Total".into()));
    assert_eq!(counts.value("C4"), Value::Number(23.0));
    assert_eq!(counts.value("H7"), Value::Number(200.0));
    let tests = &sheets[1];
    assert_eq!(tests.value("A3"), Value::Text("Pearson Chi-Square".into()));
    let XCell { value, format, .. } = tests.at(2, 1);
    assert!(
        matches!(value, Value::Number(x) if (x - 12.58).abs() < 0.005),
        "{value:?}"
    );
    assert_eq!(format, "0.00");
    assert_eq!(tests.value("C3"), Value::Number(8.0));
    assert_eq!(tests.at(2, 2).format, "0");
}

#[test]
fn descriptives_with_empty_cells() {
    let tables = tables("DESCRIPTIVES age monthly_spend.");
    let sheets = round_trip("descriptives", &tables, false);
    assert_eq!(sheets[0].name, "Descriptive Statistics");
    check_table(&sheets[0], &tables[0], 0);
    let sheet = &sheets[0];
    assert_eq!(sheet.value("C3"), Value::Number(200.0));
    assert!(matches!(sheet.value("D3"), Value::Number(m) if (m - 50.84).abs() < 0.005));
    assert_eq!(sheet.value("A5"), Value::Text("Valid N (listwise)".into()));
    assert_eq!(sheet.value("C5"), Value::Number(181.0));
    assert_eq!(sheet.value("D5"), Value::Blank);
}

#[test]
fn ctables_formats_become_excel_formats() {
    let tables = tables(
        "CTABLES /TABLE monthly_spend [SUM COMMA12.2, MEAN DOLLAR10.2, MEDIAN F8.3, \
         MAXIMUM COMMA10.0, MINIMUM DOLLAR9.0] BY gender.",
    );
    let sheets = round_trip("formats", &tables, false);
    check_table(&sheets[0], &tables[0], 0);
    let sheet = &sheets[0];
    // title, Gender, the gender categories and the statistics: the body starts on row 5
    let format = |reference: &str| sheet.cells[reference].format.clone();
    // males: sum 16978.81, mean 175.04, median 150.08, maximum 393.69, minimum 5.87
    assert!(matches!(sheet.value("B5"), Value::Number(x) if (x - 16978.81).abs() < 1e-6));
    assert_eq!(format("B5"), "#,##0.00");
    assert_eq!(format("C5"), "\"$\"#,##0.00");
    assert_eq!(format("D5"), "0.000");
    assert_eq!(format("E5"), "#,##0");
    assert_eq!(format("F5"), "\"$\"#,##0");
    assert_eq!(sheet.value("F5"), Value::Number(5.87));
}

#[test]
fn one_sheet_puts_the_tables_one_below_the_other() {
    let tables = tables(
        "FREQUENCIES gender.\n\
         CROSSTABS /TABLES=gender BY region /STATISTICS=CHISQ.\n\
         CTABLES /HIDESMALLCOUNTS COUNT=10 /TABLE region BY gender [COUNT, COLPCT.COUNT PCT40.1] \
         BY q5_1 /COMPARETEST TYPE=PROP ALPHA=0.3 ADJUST=NONE MERGE=YES.\n\
         DESCRIPTIVES age monthly_spend.",
    );
    assert_eq!(tables.len(), 6);
    let sheets = round_trip("one-sheet", &tables, true);
    assert_eq!(sheets.len(), 1);
    let sheet = &sheets[0];
    assert_eq!(sheet.name, "Tables");
    assert_eq!(sheet.pane, None);
    let mut top = 0;
    for table in &tables {
        let next = check_table(sheet, table, top);
        // one blank row between tables
        assert!((0..20).all(|c| sheet.at(next, c).value == Value::Blank));
        top = next + 1;
    }
    let widest = tables.iter().map(|t| t.columns.len()).max().unwrap();
    assert_eq!(sheet.widths.len(), widest);
}

#[test]
fn dates_times_and_currencies_stay_numbers_with_an_excel_format() {
    let source = "DATA LIST LIST /d (ADATE10) t c (F8.2).\nBEGIN DATA\n\
                  01/01/2020 3600 1\n03/15/2021 9030 2\nEND DATA.\n\
                  FORMATS t (TIME8) c (CCA8.2).\nSET CCA='(,$,,)'.\n\
                  DESCRIPTIVES d t c /STATISTICS=MIN MAX.";
    let result = Session::default().run(source, None).unwrap();
    let tables: Vec<Table> = result.commands.into_iter().flat_map(|c| c.tables).collect();
    let sheets = round_trip("dates", &tables, false);
    let cells: Vec<(Value, String)> = sheets[0]
        .cells
        .values()
        .map(|c| (c.value.clone(), c.format.clone()))
        .collect();
    let has = |value: f64, format: &str| {
        let wanted = (Value::Number(value), format.to_string());
        assert!(cells.contains(&wanted), "{wanted:?} in {cells:?}");
    };
    // 2020-01-01 is day 43 831 of Excel's calendar; one hour is 1/24 of a day
    has(43_831.0, "mm\\/dd\\/yyyy");
    has(1.0 / 24.0, "[hh]:mm:ss");
    has(1.0, "\\$#,##0.00;\\(\\$#,##0.00\\)");
}
