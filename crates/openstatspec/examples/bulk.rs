//! Speed and memory of storing and reading a large synthetic survey dataset. Not a test: run it
//! by hand against a throwaway database, one direction per process so that each peak memory is
//! its own.
//!
//! ```text
//! cargo run --release -p statscraft-openstatspec --example bulk -- KIND TARGET CASES VARIABLES store|read
//! ```
//!
//! KIND is sqlite (TARGET a file), postgres (a URL whose schema parameter names a new schema),
//! mysql or dolt (a URL whose database is new; `store` creates it). `store` writes the dataset `bulk`,
//! `read` reads it back and checks every value bit for bit. Prints one line of JSON with the
//! seconds and the peak resident memory (VmHWM) of the process.

use statscraft_data::{Dataset, Values};
use statscraft_openstatspec::catalog::{read_dataset, write_dataset};
use statscraft_openstatspec::db::Db;
use statscraft_openstatspec::{mysql, postgres, sqlite};
use std::time::Instant;

/// A deterministic hash of a cell, so that reading can check values without a second copy.
fn mix(case: usize, variable: usize) -> u64 {
    let mut x = (case as u64) << 20 ^ variable as u64 ^ 0x9e37_79b9_7f4a_7c15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// Every tenth variable is a string; of the numbers, every fifth is continuous and the rest are
/// answer codes 1 to 7; about one value in twenty is system-missing.
fn is_string(variable: usize) -> bool {
    variable % 10 == 9
}

fn number(case: usize, variable: usize) -> Option<f64> {
    let h = mix(case, variable);
    if h.is_multiple_of(20) {
        None
    } else if variable % 5 == 4 {
        Some((h >> 11) as f64 / (1u64 << 53) as f64 * 1000.0 - 500.0)
    } else {
        Some((h % 7 + 1) as f64)
    }
}

fn string(case: usize, variable: usize) -> String {
    let h = mix(case, variable);
    let words = [
        "yes",
        "no",
        "maybe, sometimes",
        "Tallinn",
        "Tartu",
        "ÄÖÜ õ",
        "it's",
    ];
    match h % 9 {
        0 => String::new(),
        n => words[n as usize % words.len()].repeat(1 + (h >> 8) as usize % 3),
    }
}

fn dataset(cases: usize, variables: usize) -> Dataset {
    let mut d = Dataset::new(cases);
    for v in 0..variables {
        let name = format!("q{}", v + 1);
        if is_string(v) {
            let values = (0..cases).map(|c| string(c, v)).collect();
            d.set_string_values(&name, 64, values).unwrap();
        } else {
            let values = (0..cases).map(|c| number(c, v)).collect();
            d.set_numeric_values(&name, values).unwrap();
        }
    }
    d
}

fn peak_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or(0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [kind, target, cases, variables, mode] = &args[..] else {
        eprintln!("usage: bulk KIND TARGET CASES VARIABLES store|read");
        std::process::exit(2);
    };
    let (cases, variables): (usize, usize) = (cases.parse().unwrap(), variables.parse().unwrap());
    let store = mode == "store";
    if store && matches!(kind.as_str(), "mysql" | "dolt") {
        use ::mysql::prelude::Queryable;
        let opts = ::mysql::Opts::from_url(target).unwrap();
        let name = opts.get_db_name().unwrap().to_string();
        let admin = ::mysql::OptsBuilder::from_opts(opts).db_name(None::<String>);
        let mut admin = ::mysql::Conn::new(admin).unwrap();
        admin.query_drop(format!("CREATE DATABASE {name}")).unwrap();
    }
    let mut db: Box<dyn Db> = match kind.as_str() {
        "sqlite" => Box::new(sqlite::open(target.as_ref(), store).unwrap()),
        "postgres" => Box::new(postgres::open(target, None, store).unwrap()),
        "mysql" => Box::new(mysql::open(target, None, store).unwrap()),
        "dolt" => Box::new(mysql::open_dolt(target, None, store).unwrap()),
        _ => panic!("unknown kind {kind}"),
    };
    let start;
    if store {
        let d = dataset(cases, variables);
        start = Instant::now();
        write_dataset(db.as_mut(), &d, "bulk").unwrap();
    } else {
        start = Instant::now();
        let (d, _) = read_dataset(db.as_mut(), "bulk", false).unwrap();
        assert_eq!((d.case_count(), d.variable_count()), (cases, variables));
        for (v, variable) in d.variables().enumerate() {
            match variable.values() {
                Values::Numeric(values) => {
                    for (c, x) in values.iter().enumerate() {
                        assert_eq!(x.map(f64::to_bits), number(c, v).map(f64::to_bits));
                    }
                }
                Values::String(values) => {
                    for (c, x) in values.iter().enumerate() {
                        assert_eq!(x, string(c, v));
                    }
                }
            }
        }
    }
    println!(
        "{{\"kind\":\"{kind}\",\"mode\":\"{mode}\",\"cases\":{cases},\"variables\":{variables},\"seconds\":{:.2},\"peak_mib\":{}}}",
        start.elapsed().as_secs_f64(),
        peak_kib() / 1024
    );
}
