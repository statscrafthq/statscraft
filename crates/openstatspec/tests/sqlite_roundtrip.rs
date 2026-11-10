//! Every fixture in tests/fixtures goes to SQLite and back, and then to a .sav file and back.
//! The dictionary and all values must be identical. The one lossy point the profile defines is
//! the compression inside a .sav file: the catalog keeps "sav" or "zsav" only, so a sav
//! dataset comes back as "bytecode" (what StatsCraft writes) and a zsav one as "zlib".

use rusqlite::Connection;
use serde_json::Value;
use statscraft_data::{Dataset, Values};
use statscraft_openstatspec::sqlite::{prepare, read_dataset, write_dataset};
use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sav" || e == "zsav"))
        .collect();
    files.sort();
    files
}

/// Compares everything the model holds except the compression, which is checked apart.
pub fn assert_same(a: &Dataset, b: &Dataset, what: &str) {
    let describe = |d: &Dataset| {
        let mut v = serde_json::to_value(d.describe()).unwrap();
        v["compression"] = Value::Null;
        v
    };
    assert_eq!(describe(a), describe(b), "{what}: describe");
    assert_eq!(a.documents(), b.documents(), "{what}: documents");
    assert_eq!(a.encoding(), b.encoding(), "{what}: encoding");
    for (x, y) in a.variables().zip(b.variables()) {
        let name = x.name();
        assert_eq!(
            x.display_width(),
            y.display_width(),
            "{what}: {name} display width"
        );
        assert_eq!(x.alignment(), y.alignment(), "{what}: {name} alignment");
        assert_eq!(x.role(), y.role(), "{what}: {name} role");
        assert_eq!(x.attributes(), y.attributes(), "{what}: {name} attributes");
        assert_eq!(
            x.value_labels(),
            y.value_labels(),
            "{what}: {name} value labels"
        );
        assert_eq!(
            x.missing_values(),
            y.missing_values(),
            "{what}: {name} missing"
        );
        match (x.values(), y.values()) {
            (Values::Numeric(p), Values::Numeric(q)) => {
                let bits = |v: Option<f64>| v.map(f64::to_bits);
                assert!(
                    p.iter().map(bits).eq(q.iter().map(bits)),
                    "{what}: {name} values"
                );
            }
            (Values::String(p), Values::String(q)) => {
                assert!(p.iter().eq(q.iter()), "{what}: {name} values")
            }
            _ => panic!("{what}: {name} changed storage kind"),
        }
    }
}

#[test]
fn every_fixture_round_trips_through_sqlite_and_back_to_sav() {
    let scratch = std::env::temp_dir().join(format!("statscraft-sqlite-rt-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let files = fixtures();
    assert_eq!(files.len(), 7, "fixtures missing");
    for path in &files {
        let what = path.file_name().unwrap().to_string_lossy().to_string();
        let original = Dataset::read_sav(path).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        prepare(&mut conn, true).unwrap();
        write_dataset(&mut conn, &original, "fixture").unwrap();
        let back = read_dataset(&mut conn, "fixture", false).unwrap();
        assert_same(&original, &back, &what);
        let zsav = what.ends_with(".zsav");
        assert_eq!(back.compression(), if zsav { "zlib" } else { "bytecode" });

        // The sav writer writes the default role "input" where the source gave none, with or
        // without SQLite in between, so the sav leg compares against a direct write.
        let reread = |d: &Dataset, file: &str| {
            let out = scratch.join(file);
            d.write_sav(&out).unwrap();
            let again = Dataset::read_sav(&out).unwrap();
            std::fs::remove_file(&out).unwrap();
            again
        };
        let direct = reread(&original, &format!("direct-{what}"));
        let via = reread(&back, &what);
        assert_same(&direct, &via, &format!("{what} via sav"));
        assert_eq!(direct.compression(), via.compression());
        println!("{what}: identical after SQLite and after SQLite plus sav");
    }
    std::fs::remove_dir(&scratch).unwrap();
}
