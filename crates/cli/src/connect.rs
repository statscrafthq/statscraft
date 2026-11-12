//! `statscraft connect`: adds this binary's MCP server to the AI agents on this computer.

use clap::Args;
use statscraft_connect::{ClientStatus, Connector, DOCS, Outcome, plain_path};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args)]
pub struct ConnectArgs {
    /// The agent: claude-desktop, claude-code, codex (also ChatGPT desktop), cursor, copilot (VS Code),
    /// gemini or antigravity. Default: every agent found on this computer
    client: Option<String>,
    /// Take Statscraft out of the agent's settings (every agent when none is named)
    #[arg(long)]
    remove: bool,
    /// Only show which agents are installed and connected; change nothing
    #[arg(long, conflicts_with_all = ["client", "remove"])]
    list: bool,
    /// Print the result as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(args: ConnectArgs) -> ExitCode {
    let server = match this_binary() {
        Ok(path) => path,
        Err(e) => {
            return fail(
                args.json,
                &format!("could not find this program's path: {e}"),
            );
        }
    };
    let connector = Connector::from_env();
    let clients = connector.clients(&server);
    if args.list {
        if args.json {
            let report = serde_json::json!({ "ok": true, "server": server, "clients": clients });
            println!("{report}");
        } else {
            print_list(&clients);
        }
        return ExitCode::SUCCESS;
    }
    let targets: Vec<String> = match &args.client {
        Some(id) => vec![id.clone()],
        None if args.remove => clients
            .iter()
            .filter(|c| c.connected)
            .map(|c| c.id.to_string())
            .collect(),
        None => clients
            .iter()
            .filter(|c| c.detected)
            .map(|c| c.id.to_string())
            .collect(),
    };
    if targets.is_empty() {
        let text = if args.remove {
            "No agent has Statscraft in its settings; nothing to remove.".to_string()
        } else {
            format!(
                "No supported AI agent was found on this computer. To connect one by hand, see {DOCS}"
            )
        };
        if args.json {
            println!(
                "{}",
                serde_json::json!({ "ok": true, "server": server, "results": [], "message": text })
            );
        } else {
            println!("{text}");
        }
        return ExitCode::SUCCESS;
    }
    if !args.json && args.client.is_none() {
        let names: Vec<&str> = clients
            .iter()
            .filter(|c| targets.iter().any(|t| t == c.id))
            .map(|c| c.name)
            .collect();
        let verb = if args.remove {
            "Removing Statscraft from"
        } else {
            "Connecting Statscraft to"
        };
        println!("{verb} {}: {}.", plural(names.len()), names.join(", "));
    }
    let results: Vec<(String, Result<Outcome, String>)> = targets
        .into_iter()
        .map(|id| {
            let done = if args.remove {
                connector.disconnect(&id)
            } else {
                connector.connect(&id, &server)
            };
            (id, done)
        })
        .collect();
    let ok = results.iter().all(|(_, r)| r.is_ok());
    if args.json {
        let results: Vec<_> = results
            .iter()
            .map(|(id, r)| match r {
                Ok(outcome) => {
                    let mut v = serde_json::to_value(outcome).expect("outcomes serialize");
                    v["ok"] = true.into();
                    v
                }
                Err(e) => serde_json::json!({ "id": id, "ok": false, "error": e }),
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({ "ok": ok, "server": server, "results": results })
        );
    } else {
        for (id, result) in &results {
            match result {
                Ok(outcome) => print_outcome(outcome, args.remove),
                Err(e) => eprintln!("{id}: error: {e}"),
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn plural(n: usize) -> String {
    if n == 1 {
        "1 agent".into()
    } else {
        format!("{n} agents")
    }
}

fn print_list(clients: &[ClientStatus]) {
    let width = clients.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in clients {
        let state = match (c.detected, &c.problem, c.connected, c.points_here) {
            (false, ..) => "not found".to_string(),
            (true, Some(problem), ..) => format!("problem: {problem}"),
            (true, None, false, _) => "found, not connected".to_string(),
            (true, None, true, true) => "connected".to_string(),
            (true, None, true, false) => "connected to another statscraft program".to_string(),
        };
        let files: Vec<String> = c.configs.iter().map(|p| p.display().to_string()).collect();
        let files = if c.detected && !files.is_empty() {
            format!("  {}", files.join(", "))
        } else {
            String::new()
        };
        println!("{:width$}  {state}{files}", c.name);
    }
}

fn print_outcome(outcome: &Outcome, remove: bool) {
    let name = outcome.name;
    if !outcome.changed {
        let state = if remove {
            "not connected"
        } else {
            "already connected"
        };
        println!("{name}: {state}, nothing changed.");
        return;
    }
    let mut did: Vec<String> = Vec::new();
    if let Some(command) = &outcome.command {
        did.push(format!("ran {command}"));
    }
    for file in &outcome.files {
        did.push(format!(
            "{} {}",
            if remove { "updated" } else { "wrote" },
            file.display()
        ));
    }
    for backup in &outcome.backups {
        did.push(format!("kept the old file as {}", backup.display()));
    }
    println!("{name}: {}.", did.join("; "));
    if let Some(restart) = outcome.restart {
        println!("  To finish: {restart}.");
    }
}

fn fail(json: bool, message: &str) -> ExitCode {
    if json {
        println!("{}", serde_json::json!({ "ok": false, "error": message }));
    } else {
        eprintln!("error: {message}");
    }
    ExitCode::FAILURE
}

/// The absolute path of this program, as agents can start it.
fn this_binary() -> std::io::Result<PathBuf> {
    Ok(plain_path(std::env::current_exe()?.canonicalize()?))
}
