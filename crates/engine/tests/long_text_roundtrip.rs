//! Long text survives a file: strings up to 1000 characters, accented and non-Latin letters,
//! a value label of 240 characters and a variable label of 240 characters are the same after
//! SAVE and GET, in the labels, the cases and the tables.
//! (A portable file writes by its own character table, so letters it has no code for are not
//! tested there.) Invented data.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "\
DATA LIST LIST(',') /ID X Y G (F8.2) S1 (A1000) S2 (A40) F (F2).
BEGIN DATA
1,3.15,10.58,1,,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,0
2,4.05,19.93,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lor,plain,1
3,6.15,9.2,3,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lore,  lead,0
4,5.47,13.23,1,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,plain,1
5,3.94,6.79,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lore,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,0
6,1.47,16.38,3,,plain,1
7,6.5,6.74,1,Lorem,trail  ,0
8,1.15,1.22,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lore,Ärevus õhtul – naeru,1
9,2.79,19.79,3,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; ,trail  ,0
10,1.9,8.46,1,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lore,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,1
11,5.32,17.64,2,,plain,0
12,8.81,17.71,3,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,plain,1
13,5.06,1.67,1,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,plain,0
14,6.35,9.39,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,1
15,5.33,12.81,3,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,0
16,8.74,19.53,1,,Ärevus õhtul – naeru,1
17,3.85,7.26,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; ,  lead,0
18,6.39,2.28,3,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; ,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,1
19,3.91,10.23,1,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lore,  lead,0
20,1.23,16.45,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,plain,1
21,1.26,0.51,3,,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,0
22,1.35,4.09,1,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; ,Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café,1
23,7.03,4.41,2,Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor sit amet; Lorem ipsum dolor si,Ärevus õhtul – naeru,0
24,3.19,8.67,3,Lorem,a;b,1
END DATA.
VARIABLE LABELS X 'A variable label that is longer than usual yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy' /Y 'short'.
VALUE LABELS G 1 'Very long value label xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx' 2 'two' 3 'Ärevus õhtul – naeru ülikool 日本語のテキスト 😀 café'.
VALUE LABELS S2 'plain' 'Very long value label xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'.
FORMATS G(F1).
";

fn run(source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?} in {source}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn same(a: &Cell, b: &Cell) -> bool {
    match (a, b) {
        (Cell::Number(x), Cell::Number(y)) => {
            (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1e-3)
        }
        (Cell::Text(x), Cell::Text(y)) => x == y,
        (Cell::Empty, Cell::Empty) => true,
        _ => false,
    }
}

const REPORTS: &str = "DISPLAY LABELS.
LIST ALL.
FREQUENCIES S1 S2 G /STATISTICS=NONE.
CODEBOOK ALL.
CROSSTABS S2 BY G.
";

fn survives(write: &str, read: &str, name: &str) {
    let dir = std::env::temp_dir().join(format!("statscraft-long-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("long.{name}"));
    let path = file.display();
    let before = run(&format!("{DATA}{REPORTS}"));
    let after = run(&format!(
        "{DATA}{}\nNEW FILE.\n{}\n{REPORTS}",
        write.replace("@FILE@", &format!("'{path}'")),
        read.replace("@FILE@", &format!("'{path}'")),
    ));
    assert_eq!(before.len(), after.len(), "{name}: number of tables");
    for (x, y) in before.iter().zip(&after) {
        assert_eq!(x.rows.len(), y.rows.len(), "{name}: {}", x.title);
        for (r, (ra, rb)) in x.rows.iter().zip(&y.rows).enumerate() {
            for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
                assert!(
                    same(ca, cb),
                    "{name}: {} row {r} column {c}: {ca:?} is not {cb:?}",
                    x.title
                );
            }
        }
    }
}

#[test]
fn long_text_survives_a_sav_file() {
    survives("SAVE OUTFILE=@FILE@.", "GET FILE=@FILE@.", "sav");
}
