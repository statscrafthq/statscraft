//! `statscraft update`: check for a newer published release, and install it.

use statscraft_engine::session::Failure;
use statscraft_update::{Client, Done, Install, current};
use std::process::ExitCode;

/// The exit code of `update --check` when a newer release exists.
const NEWER: u8 = 10;

pub fn run(check: bool, json: bool) -> Result<ExitCode, Vec<Failure>> {
    let fail = |e: String| vec![Failure::plain("update", e)];
    let client = Client::new();
    let latest = client.latest().map_err(fail)?;
    let newer = latest.as_ref().filter(|r| r.is_newer());
    if check {
        if json {
            let report = serde_json::json!({
                "current": current().to_string(),
                "latest": latest.as_ref().map(|r| r.version.to_string()),
                "update": newer.is_some(),
                "notes": newer.map(|r| &r.notes),
            });
            println!("{report}");
        } else {
            println!("current: {}", current());
            match &latest {
                Some(release) => println!("latest: {}", release.version),
                None => println!("latest: no published release yet"),
            }
            if let Some(release) = newer {
                println!("an update is available, release notes: {}", release.notes);
            }
        }
        return Ok(ExitCode::from(if newer.is_some() { NEWER } else { 0 }));
    }
    let Some(release) = newer else {
        match latest {
            Some(_) => println!("StatsCraft {} is up to date", current()),
            None => println!("no StatsCraft release has been published yet"),
        }
        return Ok(ExitCode::SUCCESS);
    };
    println!("downloading StatsCraft {}", release.version);
    match client
        .update(release, &Install::detect(), false)
        .map_err(fail)?
    {
        Done::InstallerStarted => println!(
            "the installer is updating StatsCraft to {} in the background and closes an open window first",
            release.version
        ),
        Done::Replaced(bin) => println!(
            "updated the programs in {} to StatsCraft {}; restart an open window to use it",
            bin.display(),
            release.version
        ),
        Done::Saved { file, command } => {
            println!("saved {}; install it with:\n  {command}", file.display())
        }
    }
    Ok(ExitCode::SUCCESS)
}
