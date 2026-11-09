//! Helpers shared by the integration tests.
#![allow(dead_code)]

use serde_json::Value as Json;
use statscraft_data::{Dataset, Values};
use std::{fs, path::Path, path::PathBuf};

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn fixture(name: &str) -> Dataset {
    Dataset::read_sav(fixtures().join(name)).unwrap()
}

/// Every `.sav` and `.zsav` fixture.
pub fn fixture_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = file_name(path);
            name.ends_with(".sav") || name.ends_with(".zsav")
        })
        .collect();
    files.sort();
    assert_eq!(files.len(), 7, "fixture count changed, update this number");
    files
}

pub fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_str().unwrap().to_string()
}

/// The ground truth stored next to a fixture.
pub fn sidecar(path: &Path) -> Json {
    let name = file_name(path);
    let sidecar = format!(
        "{}.expected.json",
        name.strip_suffix(".sav").unwrap_or(&name)
    );
    serde_json::from_str(&fs::read_to_string(path.with_file_name(sidecar)).unwrap()).unwrap()
}

/// A path for a file a test writes, outside the repository sources and free of what an earlier
/// run left there.
pub fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_file(&path);
    path
}

/// All values of a dataset, numbers as bit patterns so that equality is exact.
pub fn cases(data: &Dataset) -> Vec<(String, Vec<Option<u64>>, Vec<String>)> {
    data.variables()
        .map(|variable| {
            let name = variable.name().to_string();
            match variable.values() {
                Values::Numeric(values) => {
                    let bits = values.iter().map(|v| v.map(f64::to_bits)).collect();
                    (name, bits, Vec::new())
                }
                Values::String(values) => {
                    let text = values.iter().map(str::to_string).collect();
                    (name, Vec::new(), text)
                }
            }
        })
        .collect()
}

/// Writes the dataset, reads it back and checks that nothing but the compression changed.
pub fn assert_survives_write(data: &Dataset, file: &str) -> Dataset {
    let path = scratch(file);
    data.write_sav(&path).unwrap();
    let copy = Dataset::read_sav(&path).unwrap();
    let mut before = serde_json::to_value(data.describe()).unwrap();
    let after = serde_json::to_value(copy.describe()).unwrap();
    let compression = if file.ends_with(".zsav") {
        "zlib"
    } else {
        "bytecode"
    };
    assert_eq!(after["compression"], compression, "{file}");
    before["compression"] = after["compression"].clone();
    // a file cannot hold an undecided level: numeric variables are written as scale, strings as nominal
    for variable in before["variables"].as_array_mut().unwrap() {
        if variable["measure"] == "unknown" {
            variable["measure"] = if variable["type"] == "string" {
                "nominal"
            } else {
                "scale"
            }
            .into();
        }
    }
    assert_eq!(before, after, "{file}: dictionary");
    assert_eq!(cases(data), cases(&copy), "{file}: case values");
    assert!(copy.warnings().is_empty(), "{file}: {:?}", copy.warnings());
    copy
}
