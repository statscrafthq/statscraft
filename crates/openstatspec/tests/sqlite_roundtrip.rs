//! Every fixture in tests/fixtures goes to SQLite and back, and then to a .sav file and back.
//! The dictionary and all values must be identical. The one lossy point the profile defines is
//! the compression inside a .sav file: the catalog keeps "sav" or "zsav" only, so a sav
//! dataset comes back as "bytecode" (what StatsCraft writes) and a zsav one as "zlib".

mod common;

use common::{differences, via_sav};
use rusqlite::Connection;
use statscraft_data::Dataset;
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
        assert_eq!(
            differences(&original, &back),
            Vec::<String>::new(),
            "{what}"
        );
        let zsav = what.ends_with(".zsav");
        assert_eq!(back.compression(), if zsav { "zlib" } else { "bytecode" });

        // The sav writer writes the default role "input" where the source gave none, with or
        // without SQLite in between, so the sav leg compares against a direct write.
        let direct = via_sav(&original, &scratch, &format!("direct-{what}"));
        let via = via_sav(&back, &scratch, &what);
        assert_eq!(
            differences(&direct, &via),
            Vec::<String>::new(),
            "{what} via sav"
        );
        assert_eq!(direct.compression(), via.compression());
        println!("{what}: identical after SQLite and after SQLite plus sav");
    }
    std::fs::remove_dir(&scratch).unwrap();
}
