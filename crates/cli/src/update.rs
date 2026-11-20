//! `statscraft update`: check for a newer published release, and install it.

use statscraft_engine::plugin;
use statscraft_engine::session::Failure;
use statscraft_update::{Client, Done, Install, current};
use std::path::Path;
use std::process::ExitCode;

/// The exit code of `update --check` when a newer release exists.
const NEWER: u8 = 10;

/// What a plugin's update check found: its name and version, and the newer version it offers.
pub struct PluginCheck {
    pub name: String,
    pub current: String,
    pub update: Option<plugin::Update>,
}

impl PluginCheck {
    /// Asks the plugin `program` whether a newer version of it exists; installs nothing.
    pub fn of(program: &Path) -> Result<PluginCheck, String> {
        let description = plugin::describe(program)?;
        if !description.updates {
            return Err(format!(
                "plugin {} does not offer updates: get a new version where it came from",
                description.name
            ));
        }
        Ok(PluginCheck {
            update: plugin::update_check(program)?,
            name: description.name,
            current: description.version,
        })
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "plugin": self.name,
            "current": self.current,
            "latest": self.update.as_ref().map_or(&self.current, |u| &u.version),
            "update": self.update.is_some(),
            "notes": self.update.as_ref().map(|u| &u.notes),
        })
    }

    pub fn text(&self) -> String {
        let mut text = format!("plugin: {}\ncurrent: {}\n", self.name, self.current);
        match &self.update {
            Some(update) => {
                text.push_str(&format!("latest: {}\n", update.version));
                text.push_str("an update is available; Edit > Settings > Plugins installs it");
                if !update.notes.is_empty() {
                    text.push_str(&format!(", notes: {}", update.notes));
                }
                text.push('\n');
            }
            None => text.push_str("up to date\n"),
        }
        text
    }
}

/// `update --check --plugin PROGRAM`: exit code 10 when the plugin offers a newer version.
pub fn plugin(program: &Path, json: bool) -> Result<ExitCode, Vec<Failure>> {
    let check = PluginCheck::of(program).map_err(|e| vec![Failure::plain("plugin", e)])?;
    match json {
        true => println!("{}", check.json()),
        false => print!("{}", check.text()),
    }
    Ok(ExitCode::from(if check.update.is_some() {
        NEWER
    } else {
        0
    }))
}

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
            Some(_) => println!("Statscraft {} is up to date", current()),
            None => println!("no Statscraft release has been published yet"),
        }
        return Ok(ExitCode::SUCCESS);
    };
    println!("downloading Statscraft {}", release.version);
    match client.update(release, &Install::detect()).map_err(fail)? {
        // this program ends right after, so its file is free before the installer needs it
        Done::Installer(file) => {
            statscraft_update::installer(&file, false)
                .spawn()
                .map_err(|e| fail(format!("cannot start {}: {e}", file.display())))?;
            println!(
                "the installer is updating Statscraft to {} in the background and closes an open window first",
                release.version
            )
        }
        Done::Replaced(bin) => println!(
            "updated the programs in {} to Statscraft {}; restart an open window to use it",
            bin.display(),
            release.version
        ),
        Done::Saved { file, command } => {
            println!("saved {}; install it with:\n  {command}", file.display())
        }
    }
    Ok(ExitCode::SUCCESS)
}
