//! Read speed of a large synthetic file. Run with
//! `cargo test --release -p statscraft-sav --test read_speed -- --ignored --nocapture`.

use std::sync::Arc;
use std::time::Instant;

use arrow::array::{ArrayRef, Float64Builder, StringBuilder};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use statscraft_sav::{Compression, SavMetadata, read_sav, write_sav};

const ROWS: usize = 300_000;
const NUMERIC: usize = 50;

fn batch() -> (RecordBatch, SavMetadata) {
    let mut fields = Vec::new();
    let mut columns: Vec<ArrayRef> = Vec::new();
    for c in 0..NUMERIC {
        let mut b = Float64Builder::with_capacity(ROWS);
        for r in 0..ROWS {
            match (r * 7 + c) % 23 {
                0 => b.append_null(),
                // a mix of small integers (one byte in bytecode) and real numbers
                k if c % 3 == 0 => b.append_value(k as f64 * 1.37 + r as f64),
                k => b.append_value(k as f64),
            }
        }
        fields.push(Field::new(format!("v{c}"), DataType::Float64, true));
        columns.push(Arc::new(b.finish()));
    }
    let mut meta = SavMetadata::from_arrow_schema(&Schema::new(fields.clone()));
    for (name, width) in [("s8", 8), ("s40", 40), ("s300", 300)] {
        let mut b = StringBuilder::new();
        for r in 0..ROWS {
            let text = format!("row {r} õäöü ").repeat(width / 10);
            let mut end = text.len().min(width);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            b.append_value(&text[..end]);
        }
        fields.push(Field::new(name, DataType::Utf8, true));
        columns.push(Arc::new(b.finish()));
        meta.variable_names.push(name.into());
        meta.variable_formats
            .insert(name.into(), format!("A{width}"));
        meta.variable_storage_widths.insert(name.into(), width);
    }
    let batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap();
    (batch, meta)
}

#[test]
#[ignore]
fn read_speed() {
    let (batch, meta) = batch();
    for (compression, name) in [
        (Compression::None, "speed_none.sav"),
        (Compression::Bytecode, "speed_bytecode.sav"),
        (Compression::Zlib, "speed_zlib.zsav"),
    ] {
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        write_sav(&path, &batch, &meta, compression, None).unwrap();
        let size = std::fs::metadata(&path).unwrap().len();
        let best = (0..5)
            .map(|_| {
                let t = Instant::now();
                let (read, _) = read_sav(&path).unwrap();
                assert_eq!(read.num_rows(), ROWS);
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min);
        eprintln!(
            "{name}: {:.0} MB, best of 5 reads {:.3} s",
            size as f64 / 1e6,
            best
        );
        std::fs::remove_file(&path).unwrap();
    }
}
