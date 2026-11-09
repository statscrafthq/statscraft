use statscraft_data::Dataset;
use std::process::ExitCode;

const USAGE: &str = "usage:
  statscraft describe <file.sav> [--json]
  statscraft --version";

fn main() -> ExitCode {
    // shortcut: hand-rolled argument handling, switch to an argument parser when `run`, `check`
    // and sessions arrive (stage E2)
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["--version" | "-V"] => {
            println!("statscraft {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["describe", file] => describe(file, false),
        ["describe", file, "--json"] | ["describe", "--json", file] => describe(file, true),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn describe(file: &str, json: bool) -> Result<(), statscraft_data::Error> {
    let d = Dataset::read_sav(file)?.describe();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&d).expect("description is plain data")
        );
        return Ok(());
    }
    println!("{} cases, {} variables", d.case_count, d.variable_count);
    for v in &d.variables {
        println!(
            "{}\t{}\t{}\tvalid {}\t{}",
            v.name,
            v.format,
            v.measure,
            v.n_valid,
            v.label.as_deref().unwrap_or("")
        );
    }
    Ok(())
}
