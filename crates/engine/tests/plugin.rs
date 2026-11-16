//! The plugin protocol end to end: the example plugin (examples/plugin-example) is built and
//! run on a CTABLES table, and its colours and comments land on the right cells.

use statscraft_engine::plugin::{self, Description};
use statscraft_engine::session::Session;
use statscraft_engine::{Table, output_file};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds the example plugin into the workspace's target folder, so `cargo clean` removes it.
fn example_plugin() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = root.join("target/plugin-example");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args(["build", "-q", "--offline", "--manifest-path"])
        .arg(root.join("examples/plugin-example/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .status()
        .expect("cargo runs");
    assert!(status.success(), "the example plugin builds");
    let name = format!("statscraft-plugin-example{}", std::env::consts::EXE_SUFFIX);
    target.join("debug").join(name)
}

/// Made-up answers: yes is 3 of 7 in all, 2 of 3 in group 1 and 1 of 4 in group 2.
fn crosstab() -> Table {
    let mut session = Session::default();
    let source = "DATA LIST LIST /answer group (2F1).
BEGIN DATA
1 1
1 1
2 1
1 2
2 2
2 2
2 2
END DATA.
CTABLES /TABLE answer [C] [COLPCT] BY group [C]
  /CATEGORIES VARIABLES=group TOTAL=YES POSITION=BEFORE.";
    session.run(source, None).unwrap();
    let items = session.output_items("Document1").unwrap();
    output_file::leaves(items)[0].1.shown().0
}

#[test]
fn the_example_plugin_shades_cells_by_their_value() {
    let program = example_plugin();
    let description: Description = plugin::describe(&program).unwrap();
    assert_eq!(
        (description.name.as_str(), description.protocol),
        ("Example", 1)
    );
    assert_eq!(description.settings.len(), 4);
    let table = crosstab();
    let h = table.row_headers;
    let defaults = description.values(|_| None);
    let answer = plugin::decorate(&program, &defaults, &[(7, &table)]).unwrap();
    assert_eq!(answer.len(), 1);
    assert_eq!(answer[0].0, 7);
    let mut decorated = table.clone();
    let (cells, style) = answer[0].1.clone();
    decorated.set_decorations(cells, style);
    let fill = |r: usize, c: usize| decorated.decoration(r, c).and_then(|d| d.fill.clone());
    let shade = |c: &str| Some(c.to_string());
    // the groups: yes 66.7% and 25%, no 33.3% and 75%; from 25% white to 75% blue
    assert_eq!(
        [fill(0, h + 1), fill(0, h + 2), fill(1, h + 2)],
        [shade("#76acdc"), shade("#ffffff"), shade("#5b9bd5")]
    );
    // the largest of each row in its style
    let bold = |r: usize, c: usize| decorated.decoration(r, c).unwrap().bold;
    assert!(bold(0, h + 1) && !bold(0, h + 2) && bold(1, h + 2));
    // the total column is left as it is
    assert!(decorated.decoration(0, h).is_none());
    let comment = decorated.decoration(0, h + 1).unwrap().comment.as_deref();
    assert_eq!(comment, Some("n = 3"));

    // another colour from the data file attribute
    let values =
        description.values(|name| (name == "plugin_example_high").then(|| "#ff0000".into()));
    let answer = plugin::decorate(&program, &values, &[(7, &table)]).unwrap();
    assert!(
        answer[0]
            .1
            .0
            .iter()
            .any(|d| d.fill.as_deref() == Some("#ff0000"))
    );

    // the largest value's style from its attribute: italic and a font instead of bold
    let style = r#"{"italic":true,"font":"Verdana","size":7}"#;
    let values =
        description.values(|name| (name == "plugin_example_largest").then(|| style.into()));
    let answer = plugin::decorate(&program, &values, &[(7, &table)]).unwrap();
    let marked = answer[0].1.0.iter().find(|d| d.italic).unwrap();
    assert!(!marked.bold);
    assert_eq!(
        (marked.font.as_deref(), marked.size),
        (Some("Verdana"), Some(7.0))
    );
}
