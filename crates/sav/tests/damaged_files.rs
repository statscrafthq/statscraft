//! Damaged input: truncated and bit-flipped copies of every fixture must give an error or a
//! result, never a panic. Deterministic, a few thousand variants.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use statscraft_sav::read_sav_from_reader;

/// xorshift64: a fixed sequence, so every run checks the same variants.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn variants(bytes: &[u8], rng: &mut Rng) -> Vec<(String, Vec<u8>)> {
    let len = bytes.len();
    let mut out = Vec::new();
    // every cut in the first 2 KiB (header and dictionary), then spread over the rest
    let cuts = (0..len.min(2048))
        .step_by(7)
        .chain((0..60).map(|_| rng.below(len)));
    for cut in cuts {
        out.push((format!("cut at {cut}"), bytes[..cut].to_vec()));
    }
    for i in 0..300 {
        let mut copy = bytes.to_vec();
        // two thirds of the flips land in the dictionary, where lengths and counts live
        let at = if i % 3 == 2 {
            rng.below(len)
        } else {
            rng.below(len.min(4096))
        };
        let bit = rng.below(8);
        copy[at] ^= 1 << bit;
        out.push((format!("bit {bit} at {at}"), copy));
    }
    for _ in 0..80 {
        // a 4-byte field overwritten with an extreme value: counts and lengths gone wild
        let mut copy = bytes.to_vec();
        let at = rng.below(len.min(4096).saturating_sub(4).max(1));
        let value: i32 = match rng.below(4) {
            0 => -1,
            1 => i32::MAX,
            2 => i32::MIN,
            _ => rng.next() as i32,
        };
        let end = (at + 4).min(len);
        copy[at..end].copy_from_slice(&value.to_le_bytes()[..end - at]);
        out.push((format!("{value} at {at}"), copy));
    }
    out
}

#[test]
fn damaged_fixtures_never_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sav" || e == "zsav"))
        .collect();
    files.sort();
    assert!(files.len() >= 7, "fixtures not found in {}", dir.display());

    // keep the place of each panic instead of printing it
    static PLACE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    std::panic::set_hook(Box::new(|info| {
        if let (Some(at), Ok(mut place)) = (info.location(), PLACE.lock()) {
            *place = at.to_string();
        }
    }));
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let mut panics = Vec::new();
    let mut count = 0;
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        for (what, copy) in variants(&bytes, &mut rng) {
            count += 1;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = read_sav_from_reader(Cursor::new(copy));
            }));
            if let Err(panic) = result {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                let name = path.file_name().unwrap().to_string_lossy();
                let place = PLACE.lock().map(|p| p.clone()).unwrap_or_default();
                panics.push(format!("{name}, {what}: {message} at {place}"));
            }
        }
    }
    let _ = std::panic::take_hook();
    assert!(count > 3000, "only {count} variants");
    assert!(
        panics.is_empty(),
        "{} of {count} variants panicked:\n{}",
        panics.len(),
        panics.join("\n")
    );
}
