//! Variable names, labels and values with letters outside ASCII (Estonian letters, a dash, a
//! Japanese word) survive a file: a .sav file keeps all of them, and Stata, workbook and text
//! files (in UTF-8, UTF-16 and Windows-1257) keep the names and the values, which the tables
//! show as before; those formats have no variable or value labels to keep. Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const CASES: &str = "DATA LIST LIST(',') /id (F3) õhtu (F4.1) ülesanne (A30) grupp (F1).
BEGIN DATA
1,2.5,Ärevus õhtul,1
2,3.5,naeru ülikool,2
3,4.5,šokolaad žürii,1
4,5.5,Tõnis Ormisson,2
5,6.5,日本語のテキスト,1
END DATA.
";

const LABELS: &str =
    "VARIABLE LABELS õhtu 'Õhtu pikkus (tundi) – ülesande jaoks' /ülesanne 'Ülesanne: päev'.
VALUE LABELS grupp 1 'Ülemine rühm' 2 'Alumine rühm'.
";

const REPORTS: &str = "FREQUENCIES grupp ülesanne /FORMAT=NOTABLE.
MEANS õhtu BY grupp.
CROSSTABS ülesanne BY grupp.
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

fn agree(a: &[Table], b: &[Table], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: number of tables");
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.title, y.title, "{what}");
        assert_eq!(x.rows.len(), y.rows.len(), "{what}: {}", x.title);
        for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
            for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                let same = match (ca, cb) {
                    (Cell::Number(p), Cell::Number(q)) => (p - q).abs() < 1e-9,
                    (a, b) => a == b,
                };
                assert!(
                    same,
                    "{what}: {} row {r} column {c}: {ca:?} is not {cb:?}",
                    x.title
                );
            }
        }
    }
}

fn survives(labels: &str, write: &str, read: &str, name: &str) {
    let dir = std::env::temp_dir().join(format!("statscraft-uni-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("u.{name}")).display().to_string();
    let fill = |text: &str| text.replace("@FILE@", &format!("'{path}'"));
    let before = run(&format!("{CASES}{labels}DISPLAY LABELS.\n{REPORTS}"));
    let after = run(&format!(
        "{CASES}{labels}{}\nNEW FILE.\n{}\nDISPLAY LABELS.\n{REPORTS}",
        fill(write),
        fill(read)
    ));
    agree(&before, &after, name);
}

#[test]
fn a_sav_file_keeps_names_labels_and_values() {
    survives(LABELS, "SAVE OUTFILE=@FILE@.", "GET FILE=@FILE@.", "sav");
}

#[test]
fn a_stata_file_keeps_names_and_values() {
    survives(
        "",
        "SAVE TRANSLATE OUTFILE=@FILE@ /TYPE=STATA /REPLACE.",
        "GET STATA FILE=@FILE@.",
        "dta",
    );
}

#[test]
fn a_workbook_keeps_names_and_values() {
    survives(
        "",
        "SAVE TRANSLATE OUTFILE=@FILE@ /TYPE=XLS /FIELDNAMES /REPLACE.",
        "GET DATA /TYPE=XLSX /FILE=@FILE@ /READNAMES=ON.",
        "xlsx",
    );
}

#[test]
fn text_files_keep_names_and_values_in_utf8_and_utf16() {
    for (encoding, name) in [("UTF8", "u8.csv"), ("UTF16LE", "u16.csv")] {
        survives(
            "",
            &format!(
                "SAVE TRANSLATE OUTFILE=@FILE@ /TYPE=CSV /FIELDNAMES /ENCODING='{encoding}' /REPLACE."
            ),
            &format!(
                "GET DATA /TYPE=TXT /FILE=@FILE@ /ENCODING='{encoding}' /DELIMITERS=',' /FIRSTCASE=2 /VARIABLES=id F3 õhtu F4.1 ülesanne A30 grupp F1."
            ),
            name,
        );
    }
}

#[test]
fn a_text_file_in_windows_1257_keeps_the_estonian_letters() {
    let cases = CASES.replace("5,6.5,日本語のテキスト,1\n", "5,6.5,Jüri Müürsepp,1\n");
    let dir = std::env::temp_dir().join(format!("statscraft-uni-1257-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("u.csv").display().to_string();
    let before = run(&format!("{cases}{REPORTS}"));
    let after = run(&format!(
        "{cases}SAVE TRANSLATE OUTFILE='{path}' /TYPE=CSV /FIELDNAMES /ENCODING='windows-1257' /REPLACE.\nNEW FILE.\nGET DATA /TYPE=TXT /FILE='{path}' /ENCODING='windows-1257' /DELIMITERS=',' /FIRSTCASE=2 /VARIABLES=id F3 õhtu F4.1 ülesanne A30 grupp F1.\n{REPORTS}"
    ));
    agree(&before, &after, "windows-1257");
}
