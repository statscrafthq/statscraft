//! What the window remembers between starts: its size and position, the folder of the last
//! file dialog, the recent files and the update check. Kept in the user's config folder as
//! lines of text.

use std::path::{Path, PathBuf};

/// The most recent files kept.
const RECENT: usize = 10;

#[derive(Default, Debug, PartialEq)]
pub struct Settings {
    pub size: Option<[f32; 2]>,
    pub position: Option<[f32; 2]>,
    pub folder: Option<PathBuf>,
    /// Data and syntax files, the most recent first.
    pub recent: Vec<PathBuf>,
    /// Automatic update checks on or off; `None` until the window has said that they exist.
    pub updates: Option<bool>,
    /// When updates were last checked, in seconds since 1970.
    pub update_checked: Option<u64>,
    /// The grids, the syntax text and the output follow the system's theme; otherwise they
    /// stay light like paper.
    pub paper_follows_system: bool,
    /// Widths of the Variable View's columns, when they were changed.
    pub variable_widths: Vec<f32>,
}

impl Settings {
    /// The settings file: `statscraft/window.txt` in `%APPDATA%` on Windows, in
    /// `$XDG_CONFIG_HOME` or `~/.config` elsewhere.
    pub fn file() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|d| !d.is_empty())
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        base.map(|b| b.join("statscraft").join("window.txt"))
    }

    /// The saved settings; nothing when there are none or they cannot be read.
    pub fn load() -> Self {
        Self::file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map_or_else(Self::default, |text| Self::parse(&text))
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(file) = Self::file() else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, self.text())
    }

    /// Reads the lines `size W H`, `position X Y`, `folder PATH`, `recent PATH`,
    /// `updates on|off`, `update-checked SECONDS`, `paper system` and `variable_widths W ...`;
    /// anything else is skipped.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        let pair = |rest: &str| {
            let numbers: Vec<f32> = rest
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            <[f32; 2]>::try_from(numbers).ok()
        };
        for line in text.lines() {
            let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "size" => settings.size = pair(rest),
                "position" => settings.position = pair(rest),
                "paper" => settings.paper_follows_system = rest == "system",
                "variable_widths" => {
                    let widths = rest.split_whitespace().map(str::parse::<f32>);
                    settings.variable_widths = widths.collect::<Result<_, _>>().unwrap_or_default();
                }
                "folder" if !rest.is_empty() => settings.folder = Some(PathBuf::from(rest)),
                "recent" if !rest.is_empty() && settings.recent.len() < RECENT => {
                    settings.recent.push(PathBuf::from(rest))
                }
                "updates" if rest == "on" || rest == "off" => settings.updates = Some(rest == "on"),
                "update-checked" => settings.update_checked = rest.parse().ok(),
                _ => {}
            }
        }
        settings
    }

    pub fn text(&self) -> String {
        let mut text = String::new();
        if let Some([w, h]) = self.size {
            text.push_str(&format!("size {w} {h}\n"));
        }
        if let Some([x, y]) = self.position {
            text.push_str(&format!("position {x} {y}\n"));
        }
        if let Some(folder) = &self.folder {
            text.push_str(&format!("folder {}\n", folder.display()));
        }
        if self.paper_follows_system {
            text.push_str("paper system\n");
        }
        if !self.variable_widths.is_empty() {
            let widths: Vec<String> = self.variable_widths.iter().map(f32::to_string).collect();
            text.push_str(&format!("variable_widths {}\n", widths.join(" ")));
        }
        for file in &self.recent {
            text.push_str(&format!("recent {}\n", file.display()));
        }
        if let Some(on) = self.updates {
            text.push_str(if on { "updates on\n" } else { "updates off\n" });
        }
        if let Some(time) = self.update_checked {
            text.push_str(&format!("update-checked {time}\n"));
        }
        text
    }

    /// Notes the folder of a file chosen in a file dialog.
    pub fn remember_folder(&mut self, file: &Path) {
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        if let Some(dir) = file.parent() {
            self.folder = Some(dir.to_path_buf());
        }
    }

    /// Puts a file opened or saved first in the recent files.
    pub fn add_recent(&mut self, file: &Path) {
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        self.recent.retain(|f| *f != file);
        self.recent.insert(0, file);
        self.recent.truncate(RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        let mut settings = Settings {
            size: Some([1200.0, 800.5]),
            position: Some([-10.0, 40.0]),
            folder: Some(PathBuf::from("/data/my surveys")),
            recent: Vec::new(),
            updates: Some(false),
            update_checked: Some(1_790_000_000),
            paper_follows_system: true,
            variable_widths: vec![150.0, 80.5],
        };
        for i in 0..12 {
            settings.add_recent(Path::new(&format!("/data/f{i}.sav")));
        }
        settings.add_recent(Path::new("/data/f5.sav"));
        assert_eq!(settings.recent.len(), RECENT);
        // add_recent stores absolute paths, and on Windows "/data" gains the current drive
        let absolute = |p: &str| std::path::absolute(p).unwrap();
        assert_eq!(settings.recent[0], absolute("/data/f5.sav"));
        assert_eq!(settings.recent[1], absolute("/data/f11.sav"));
        assert_eq!(Settings::parse(&settings.text()), settings);
        // broken or unknown lines are skipped
        let read = Settings::parse(
            "size 10\nposition a b\ncolour red\nfolder \nrecent /x.sps\nupdates maybe\n",
        );
        assert_eq!(
            read,
            Settings {
                recent: vec![PathBuf::from("/x.sps")],
                ..Default::default()
            }
        );
        settings.remember_folder(Path::new("/elsewhere/job.sps"));
        assert_eq!(settings.folder, Some(absolute("/elsewhere")));
        // a relative path is kept as the folder it means now
        settings.remember_folder(Path::new("job.sps"));
        assert_eq!(settings.folder, Some(std::env::current_dir().unwrap()));
    }
}
