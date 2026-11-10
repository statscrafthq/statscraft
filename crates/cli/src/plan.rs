//! `statscraft plan`: lowers bounded syntax to an OpenStatSpec transformation plan.

use crate::{RunArgs, load_session, read_source};
use clap::Args;
use serde_json::{Value, json};
use statscraft_data::Dataset;
use statscraft_openstatspec::plan::Plan;
use statscraft_openstatspec::{FRONTEND_CONTRACTS, apply, canonical_json, compile_request, sqlite};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args)]
#[command(after_help = "\
Prints one line of canonical JSON: {\"ok\":true,\"plan\":...,\"plan_hash\":...,\"source_hash\":...}, \
or {\"ok\":false,\"error\":{\"code\":...,\"message\":...,\"line\":...,\"column\":...},...}. \
The plan hash is SHA-256 of the canonical plan; the source hash is SHA-256 of the source with \
line ends converted to LF.

Profile 0.1: RECODE, VARIABLE LABELS, VALUE LABELS.
Profile 0.2 adds COMPUTE target = operand, IF (predicate) target = operand, FORMATS (F only), \
VARIABLE LEVEL and EXECUTE.
Profile 0.3 adds comments, TO lists, NE <> ~= and NOT, LOWEST/HIGHEST ranges and ADD VALUE LABELS.
Anything else is rejected: this is OpenStatSpec's bounded frontend, not the full language of `statscraft run`.

With --sqlite DB --name DATASET the input schema comes from that OpenStatSpec dataset, and \
--apply applies the plan to it in place, in one transaction with one audit row; the output \
then has \"applied\":{\"apply_id\":...,\"dataset_id\":...,\"operation_count\":...}.")]
pub struct PlanArgs {
    /// Syntax file, or - for standard input
    file: Option<String>,
    /// Syntax text given directly
    #[arg(short = 'e', long = "execute", conflicts_with = "file")]
    text: Option<String>,
    /// Input schema: a JSON file with {"variables": [{"name", "storage_kind", ...}]}
    #[arg(long, value_name = "FILE", conflicts_with_all = ["session", "sqlite"])]
    input_schema: Option<PathBuf>,
    /// Take the input schema from the dataset of this session
    #[arg(short, long, conflicts_with = "sqlite")]
    session: Option<String>,
    /// OpenStatSpec SQLite database that holds the dataset named by --name
    #[arg(long, value_name = "DB", requires = "name")]
    sqlite: Option<PathBuf>,
    /// Dataset name in the --sqlite database
    #[arg(long, value_name = "DATASET", requires = "sqlite")]
    name: Option<String>,
    /// Apply the plan to the --sqlite dataset in place
    #[arg(long, requires = "sqlite")]
    apply: bool,
    /// Who applies the plan, for the audit row; defaults to $USER
    #[arg(long)]
    actor: Option<String>,
    /// Frontend profile version
    #[arg(long, default_value = "0.3", value_parser = ["0.1", "0.2", "0.3"])]
    profile: String,
    /// Symbolic name of the input dataset in the plan
    #[arg(long, default_value = "input")]
    alias: String,
}

pub fn run(args: PlanArgs) -> ExitCode {
    let output = match request(&args) {
        Ok(request) => {
            let out = compile_request(&request);
            match out.result {
                Ok(c) => {
                    let mut ok = json!({
                        "ok": true,
                        "plan": c.plan.to_json(),
                        "plan_hash": c.plan_hash,
                        "source_hash": out.source_hash,
                    });
                    if args.apply {
                        let source_hash = out.source_hash.as_deref().unwrap_or("");
                        match apply_to_sqlite(&args, &c.plan, source_hash) {
                            Ok(applied) => ok["applied"] = applied,
                            Err(e) => {
                                ok["ok"] = json!(false);
                                ok["error"] = json!({"code": e.code, "message": e.message});
                            }
                        }
                    }
                    ok
                }
                Err(d) => {
                    let mut error = json!({"code": d.code, "message": d.message});
                    if let Some(s) = d.span {
                        for (key, n) in [
                            ("line", s.line),
                            ("column", s.column),
                            ("start", s.start),
                            ("end", s.end),
                        ] {
                            error[key] = json!(n);
                        }
                    }
                    json!({"ok": false, "error": error, "source_hash": out.source_hash})
                }
            }
        }
        Err(message) => json!({"ok": false, "error": {"code": "usage", "message": message}}),
    };
    println!(
        "{}",
        canonical_json(&output).expect("plain data with small integers")
    );
    if output["ok"] == true {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn request(args: &PlanArgs) -> Result<Value, String> {
    let first_message = |failures: Vec<crate::Failure>| {
        failures
            .into_iter()
            .next()
            .map_or_else(String::new, |f| f.message)
    };
    let source = read_source(&RunArgs {
        file: args.file.clone(),
        text: args.text.clone(),
        session: None,
        lines: None,
        json: false,
    })
    .map_err(first_message)?;
    let v01 = args.profile == "0.1";
    let input_schema = match (&args.input_schema, &args.session) {
        (Some(path), None) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        (None, Some(name)) => {
            let dataset = load_session(name)
                .map_err(first_message)?
                .ok_or_else(|| format!("session {name} has no dataset yet"))?;
            schema_of(&dataset, !v01)
        }
        (None, None) => match (&args.sqlite, &args.name) {
            (Some(path), Some(name)) => sqlite::open(path, false)
                .and_then(|conn| sqlite::input_schema(&conn, name, !v01))
                .map_err(|e| e.to_string())?,
            _ => {
                return Err(
                    "give --input-schema FILE, --session NAME or --sqlite DB --name DATASET".into(),
                );
            }
        },
        _ => return Err("give --input-schema FILE or --session NAME".into()),
    };
    let contract = FRONTEND_CONTRACTS[["0.1", "0.2", "0.3"]
        .iter()
        .position(|p| *p == args.profile)
        .expect("clap checks the profile")];
    Ok(json!({
        "contract": contract,
        "input_alias": args.alias,
        "input_schema": input_schema,
        "source_text": source,
    }))
}

/// Applies a compiled plan to the --sqlite dataset; an empty actor fails in the binding.
fn apply_to_sqlite(
    args: &PlanArgs,
    plan: &Plan,
    source_hash: &str,
) -> Result<Value, sqlite::DbError> {
    let (Some(path), Some(name)) = (&args.sqlite, &args.name) else {
        return Err(sqlite::DbError::new(
            "usage",
            "--apply needs --sqlite DB --name DATASET",
        ));
    };
    let actor = args
        .actor
        .clone()
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default();
    let mut conn = sqlite::open(path, false)?;
    let applied = apply::apply_plan(&mut conn, name, plan, source_hash, &actor)?;
    Ok(json!({
        "apply_id": applied.apply_id,
        "dataset_id": applied.dataset_id,
        "operation_count": applied.operation_count,
    }))
}

/// The input schema of a dataset; the format and measurement fields exist from profile 0.2 on.
fn schema_of(dataset: &Dataset, with_format: bool) -> Value {
    let variables: Vec<Value> = dataset
        .variables()
        .map(|v| {
            let mut out = json!({
                "name": v.name(),
                "storage_kind": if v.is_string() { "string" } else { "numeric" },
                "variable_label": v.label(),
                "value_labels": v.value_labels().iter().map(|(value, label)| {
                    let value = match value {
                        // positive zero is the canonical zero
                        statscraft_data::Value::Number(n) => json!({
                            "type": "binary64",
                            "bits": format!("{:016x}", if *n == 0.0 { 0 } else { n.to_bits() }),
                        }),
                        statscraft_data::Value::String(s) => json!({"type": "string", "value": s}),
                    };
                    json!({"value": value, "label": label})
                }).collect::<Vec<_>>(),
            });
            if with_format {
                // "F8.2" gives family F, width 8, decimals 2
                let format = v.format();
                let digits = format
                    .find(|c: char| c.is_ascii_digit())
                    .unwrap_or(format.len());
                let (family, size) = format.split_at(digits);
                let (width, decimals) = size.split_once('.').unwrap_or((size, ""));
                out["format_family"] = json!(family);
                out["width"] = json!(width.parse::<u64>().ok());
                out["decimals"] = json!(decimals.parse::<u64>().ok());
                out["measurement_level"] = match v.measure() {
                    statscraft_data::Measure::Unknown => Value::Null,
                    m => json!(m.as_str()),
                };
            }
            out
        })
        .collect();
    json!({ "variables": variables })
}
