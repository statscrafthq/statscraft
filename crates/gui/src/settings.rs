//! What the window remembers between starts: its size and position, the folder of the last
//! file dialog, the recent files and the settings of Edit > Settings. Kept in the user's config
//! folder as lines of text. How tables show variables and values (SET TVARS and TNUMBERS) and
//! the parts of multiply imputed output (SET MIOUTPUT) are kept here too, and a new session of
//! the window starts by running that SET.

use crate::output::{Part, Show};
use std::path::{Path, PathBuf};

/// The most recent files kept.
const RECENT: usize = 10;

/// The values of SET TVARS and TNUMBERS that the window keeps.
const TVARS: [&str; 3] = ["NAMES", "LABELS", "BOTH"];
const TNUMBERS: [&str; 3] = ["VALUES", "LABELS", "BOTH"];
/// The parts SET MIOUTPUT can show for multiply imputed data.
pub const MIOUTPUT: [&str; 4] = ["OBSERVED", "IMPUTED", "POOLED", "DIAGNOSTICS"];

#[derive(Default, Debug, PartialEq)]
pub struct Settings {
    /// The size and position of the last data window (the first window of a start).
    pub size: Option<[f32; 2]>,
    pub position: Option<[f32; 2]>,
    /// Of the last syntax window and output window.
    pub syntax_size: Option<[f32; 2]>,
    pub syntax_position: Option<[f32; 2]>,
    pub output_size: Option<[f32; 2]>,
    pub output_position: Option<[f32; 2]>,
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
    /// The colour theme of the syntax editor, by name; empty for the default.
    pub theme: String,
    /// Long lines of the syntax editor stay on one line instead of wrapping.
    pub wrap_off: bool,
    /// No log under the Syntax and Output views (a run with warnings still shows it).
    pub log_hidden: bool,
    /// SET TVARS and TNUMBERS chosen in Edit > Settings, in upper case; `None` until chosen.
    pub tvars: Option<String>,
    pub tnumbers: Option<String>,
    /// SET MIOUTPUT chosen in Edit > Settings: its keywords in upper case, space separated.
    pub mioutput: Option<String>,
    /// The kinds of element the output shows, and whether exports take hidden items.
    pub output: Show,
    /// The plugin program that decorates tables (docs/plugins.md), and whether it is off.
    pub plugin: Option<PathBuf>,
    pub plugin_off: bool,
    /// Excel exports leave out the plugin's comments.
    pub excel_comments_off: bool,
    /// Saved values of plugin settings, for every data file.
    pub presets: Vec<Preset>,
    /// The plugin settings last chosen for each data file, by its path: they hold when the
    /// file is read again (GET FILE in a syntax) before its new data file attributes were saved.
    pub file_values: Vec<FileValue>,
    /// Design templates of the plugin's settings, and the name of the one in use.
    pub templates: Vec<Template>,
    pub template: Option<String>,
}

/// A named set of plugin setting values (every setting not kept per data file), as
/// (attribute, value) pairs; one template per client's look, say.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub name: String,
    pub values: Vec<(String, String)>,
}

impl Template {
    /// A template from the dialog: tabs and line breaks become spaces, so each value fits its
    /// line of the settings file.
    pub fn new(name: &str, values: &[(String, String)]) -> Self {
        let values = values.iter().map(|(a, v)| (clean(a), clean(v)));
        Template {
            name: clean(name).trim().to_string(),
            values: values.collect(),
        }
    }
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// A plugin setting's value chosen for one data file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileValue {
    pub file: PathBuf,
    pub attribute: String,
    pub value: String,
}

/// A saved value of a plugin setting, named, under the setting's data file attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub attribute: String,
    pub name: String,
    pub value: String,
}

impl Preset {
    /// A preset from the dialog: tabs and line breaks in the name or value become spaces, so
    /// it fits its line of the settings file.
    pub fn new(attribute: &str, name: &str, value: &str) -> Self {
        let clean = |s: &str| s.trim().replace(['\t', '\n', '\r'], " ");
        Preset {
            attribute: attribute.to_string(),
            name: clean(name),
            value: clean(value),
        }
    }
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

    /// The size and position a window of a kind (data, syntax or output) opens with.
    pub fn window(&self, kind: &str) -> (Option<[f32; 2]>, Option<[f32; 2]>) {
        match kind {
            "syntax" => (self.syntax_size, self.syntax_position),
            "output" => (self.output_size, self.output_position),
            _ => (self.size, self.position),
        }
    }

    pub fn window_mut(&mut self, kind: &str) -> (&mut Option<[f32; 2]>, &mut Option<[f32; 2]>) {
        match kind {
            "syntax" => (&mut self.syntax_size, &mut self.syntax_position),
            "output" => (&mut self.output_size, &mut self.output_position),
            _ => (&mut self.size, &mut self.position),
        }
    }

    /// Reads the lines `size W H`, `position X Y` (of the data window; `syntax-size`,
    /// `syntax-position`, `output-size` and `output-position` of the others), `folder PATH`,
    /// `recent PATH`,
    /// `updates on|off`, `update-checked SECONDS`, `paper system`, `theme NAME`, `wrap off`,
    /// `log hidden`, `variable_widths W ...`, `tvars names|labels|both` and
    /// `tnumbers values|labels|both`, `mioutput` with MIOUTPUT keywords, `output-shows` with
    /// the kinds of element the output shows (`headings titles tables notes charts`) and
    /// `export hidden`, `plugin PATH`, `plugin-off`, `excel-comments off` and
    /// `preset ATTRIBUTE<tab>NAME<tab>VALUE`, `plugin-value ATTRIBUTE<tab>VALUE<tab>FILE`,
    /// `template NAME<tab>ATTRIBUTE<tab>VALUE` (or `template NAME` for one without values) and
    /// `template-active NAME`; anything else is skipped.
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
                "syntax-size" => settings.syntax_size = pair(rest),
                "syntax-position" => settings.syntax_position = pair(rest),
                "output-size" => settings.output_size = pair(rest),
                "output-position" => settings.output_position = pair(rest),
                "paper" => settings.paper_follows_system = rest == "system",
                "theme" => settings.theme = rest.to_string(),
                "wrap" => settings.wrap_off = rest == "off",
                "log" => settings.log_hidden = rest == "hidden",
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
                "tvars" => settings.tvars = keyword(&TVARS, rest),
                "tnumbers" => settings.tnumbers = keyword(&TNUMBERS, rest),
                "mioutput" => settings.mioutput = mi_parts(rest),
                "output-shows" => {
                    let words: Vec<&str> = rest.split_whitespace().collect();
                    settings.output.kinds = Part::ALL.map(|p| words.contains(&p.word()));
                }
                "export" => settings.output.export_hidden = rest == "hidden",
                "plugin" if !rest.is_empty() => settings.plugin = Some(PathBuf::from(rest)),
                "plugin-off" => settings.plugin_off = true,
                "excel-comments" => settings.excel_comments_off = rest == "off",
                "preset" => {
                    if let [attribute, name, value] = rest.split('\t').collect::<Vec<_>>()[..]
                        && !name.is_empty()
                    {
                        settings.presets.push(Preset::new(attribute, name, value));
                    }
                }
                "plugin-value" => {
                    if let [attribute, value, file] = rest.split('\t').collect::<Vec<_>>()[..]
                        && !attribute.is_empty()
                        && !file.is_empty()
                    {
                        settings.file_values.push(FileValue {
                            file: PathBuf::from(file),
                            attribute: attribute.to_string(),
                            value: value.to_string(),
                        });
                    }
                }
                "template" => {
                    let parts: Vec<&str> = rest.split('\t').collect();
                    if parts[0].is_empty() {
                        continue;
                    }
                    let found = settings.templates.iter().position(|t| t.name == parts[0]);
                    let at = found.unwrap_or_else(|| {
                        settings.templates.push(Template::new(parts[0], &[]));
                        settings.templates.len() - 1
                    });
                    if let [_, attribute, value] = parts[..]
                        && !attribute.is_empty()
                    {
                        let pair = (attribute.to_string(), value.to_string());
                        settings.templates[at].values.push(pair);
                    }
                }
                "template-active" if !rest.is_empty() => settings.template = Some(rest.into()),
                _ => {}
            }
        }
        settings
    }

    pub fn text(&self) -> String {
        let mut text = String::new();
        for (key, pair) in [
            ("size", self.size),
            ("position", self.position),
            ("syntax-size", self.syntax_size),
            ("syntax-position", self.syntax_position),
            ("output-size", self.output_size),
            ("output-position", self.output_position),
        ] {
            if let Some([a, b]) = pair {
                text.push_str(&format!("{key} {a} {b}\n"));
            }
        }
        if let Some(folder) = &self.folder {
            text.push_str(&format!("folder {}\n", folder.display()));
        }
        if self.paper_follows_system {
            text.push_str("paper system\n");
        }
        if !self.theme.is_empty() {
            text.push_str(&format!("theme {}\n", self.theme));
        }
        if self.wrap_off {
            text.push_str("wrap off\n");
        }
        if self.log_hidden {
            text.push_str("log hidden\n");
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
        for (key, value) in [("tvars", &self.tvars), ("tnumbers", &self.tnumbers)] {
            if let Some(value) = value {
                text.push_str(&format!("{key} {}\n", value.to_lowercase()));
            }
        }
        if let Some(parts) = &self.mioutput {
            text.push_str(&format!("mioutput {}\n", parts.to_lowercase()));
        }
        if self.output.kinds != Show::default().kinds {
            text.push_str("output-shows");
            for part in Part::ALL.into_iter().filter(|p| self.output.kind(*p)) {
                text.push_str(&format!(" {}", part.word()));
            }
            text.push('\n');
        }
        if self.output.export_hidden {
            text.push_str("export hidden\n");
        }
        if let Some(plugin) = &self.plugin {
            text.push_str(&format!("plugin {}\n", plugin.display()));
        }
        if self.plugin_off {
            text.push_str("plugin-off\n");
        }
        if self.excel_comments_off {
            text.push_str("excel-comments off\n");
        }
        for p in &self.presets {
            text.push_str(&format!(
                "preset {}\t{}\t{}\n",
                p.attribute, p.name, p.value
            ));
        }
        for v in &self.file_values {
            text.push_str(&format!(
                "plugin-value {}\t{}\t{}\n",
                v.attribute,
                v.value,
                v.file.display()
            ));
        }
        for t in &self.templates {
            if t.values.is_empty() {
                text.push_str(&format!("template {}\n", t.name));
            }
            for (attribute, value) in &t.values {
                text.push_str(&format!("template {}\t{attribute}\t{value}\n", t.name));
            }
        }
        if let Some(name) = &self.template {
            text.push_str(&format!("template-active {name}\n"));
        }
        text
    }

    /// The design template in use, if any.
    pub fn active_template(&self) -> Option<&Template> {
        let name = self.template.as_ref()?;
        self.templates.iter().find(|t| &t.name == name)
    }

    /// Keeps the plugin settings chosen for a data file, replacing earlier ones of the same
    /// attributes; tabs and line breaks become spaces so each fits its line.
    pub fn remember_values(&mut self, file: &Path, values: &[(String, String)]) {
        for (attribute, value) in values {
            self.file_values
                .retain(|v| !(v.file == file && v.attribute.eq_ignore_ascii_case(attribute)));
            self.file_values.push(FileValue {
                file: file.to_path_buf(),
                attribute: clean(attribute),
                value: clean(value),
            });
        }
    }

    /// The plugin settings remembered for a data file, as data file attributes.
    pub fn values_for(&self, file: &Path) -> Vec<(String, Vec<String>)> {
        let mine = self.file_values.iter().filter(|v| v.file == file);
        mine.map(|v| (v.attribute.clone(), vec![v.value.clone()]))
            .collect()
    }

    /// Keeps a choice of SET TVARS or TNUMBERS from Edit > Settings.
    pub fn keep_table_labels(&mut self, subcommand: &str, value: &str) {
        match subcommand {
            "TVARS" => self.tvars = keyword(&TVARS, value),
            "TNUMBERS" => self.tnumbers = keyword(&TNUMBERS, value),
            _ => {}
        }
    }

    /// Keeps a choice of SET MIOUTPUT from Edit > Settings; a list SET would refuse is not kept.
    pub fn keep_mioutput(&mut self, parts: &str) {
        self.mioutput = mi_parts(parts);
    }

    /// The SET command a new session of the window starts with, when TVARS, TNUMBERS or
    /// MIOUTPUT was chosen; the session then holds them like any SET.
    pub fn startup_syntax(&self) -> Option<String> {
        let mut set: Vec<String> = [("TVARS", &self.tvars), ("TNUMBERS", &self.tnumbers)]
            .into_iter()
            .filter_map(|(name, value)| Some(format!("{name}={}", value.as_ref()?)))
            .collect();
        set.extend(self.mioutput.as_ref().map(|p| format!("MIOUTPUT=[{p}]")));
        (!set.is_empty()).then(|| format!("SET {}.", set.join(" ")))
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

/// MIOUTPUT keywords in upper case and in their usual order; None when none or an unknown one.
fn mi_parts(text: &str) -> Option<String> {
    let words: Vec<String> = text.split_whitespace().map(str::to_uppercase).collect();
    let known = words.iter().all(|w| MIOUTPUT.contains(&w.as_str()));
    let parts: Vec<&str> = MIOUTPUT
        .into_iter()
        .filter(|k| words.iter().any(|w| w == k))
        .collect();
    (known && !parts.is_empty()).then(|| parts.join(" "))
}

/// `word` in upper case when it is one of `allowed`.
fn keyword(allowed: &[&str], word: &str) -> Option<String> {
    let word = word.trim().to_uppercase();
    allowed.contains(&word.as_str()).then_some(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        let mut settings = Settings {
            size: Some([1200.0, 800.5]),
            position: Some([-10.0, 40.0]),
            syntax_size: Some([900.0, 600.0]),
            syntax_position: None,
            output_size: Some([1000.0, 700.0]),
            output_position: Some([30.0, 60.0]),
            folder: Some(PathBuf::from("/data/my surveys")),
            recent: Vec::new(),
            updates: Some(false),
            update_checked: Some(1_790_000_000),
            paper_follows_system: true,
            variable_widths: vec![150.0, 80.5],
            theme: "Solarized Dark".into(),
            wrap_off: true,
            log_hidden: true,
            tvars: Some("NAMES".into()),
            tnumbers: Some("BOTH".into()),
            mioutput: Some("IMPUTED POOLED".into()),
            output: Show {
                kinds: [true, false, true, false, true],
                export_hidden: true,
            },
            plugin: Some(PathBuf::from("/opt/plugins/heat map")),
            plugin_off: true,
            excel_comments_off: true,
            presets: vec![
                Preset::new("plugin_house_limit", "Strict 1-10", "1043000"),
                Preset::new("plugin_house_limit", "All: steps\t0+", "1331000"),
                Preset::new("plugin_house_note", "Short", "Base: all asked"),
            ],
            file_values: vec![FileValue {
                file: PathBuf::from("/data/my surveys/wave 2.sav"),
                attribute: "plugin_house_limit".into(),
                value: "1043000".into(),
            }],
            templates: vec![
                Template::new(
                    "Client A",
                    &[
                        ("plugin_house_high".into(), "#ed7d31".into()),
                        ("plugin_house_head".into(), "{\"bold\":true}\tx".into()),
                    ],
                ),
                Template::new("Empty one", &[]),
            ],
            template: Some("Client A".into()),
        };
        assert_eq!(settings.templates[0].values[1].1, "{\"bold\":true} x");
        assert_eq!(settings.active_template(), Some(&settings.templates[0]));
        assert_eq!(settings.presets[1].name, "All: steps 0+");
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

    #[test]
    fn what_the_output_shows_is_kept() {
        // the defaults write nothing
        let mut settings = Settings::default();
        assert_eq!(settings.text(), "");
        assert!(!settings.output.kind(Part::Heading) && settings.output.kind(Part::Title));
        settings.output.kinds = [true, false, true, true, true];
        settings.output.export_hidden = true;
        let text = settings.text();
        assert_eq!(
            text,
            "output-shows headings tables notes charts\nexport hidden\n"
        );
        assert_eq!(Settings::parse(&text), settings);
        // every kind off is kept too
        settings.output.kinds = [false; 5];
        assert_eq!(settings.text(), "output-shows\nexport hidden\n");
        assert_eq!(Settings::parse(&settings.text()), settings);
    }

    #[test]
    fn table_labels_are_kept_and_set_when_a_session_starts() {
        let mut settings = Settings::default();
        assert_eq!(settings.startup_syntax(), None);
        settings.keep_table_labels("TVARS", "NAMES");
        settings.keep_table_labels("TNUMBERS", "both");
        let text = settings.text();
        assert_eq!(text, "tvars names\ntnumbers both\n");
        let read = Settings::parse(&text);
        assert_eq!(read, settings);
        // a value SET would refuse is not kept
        assert_eq!(
            Settings::parse("tvars values\ntnumbers names\n"),
            Settings::default()
        );
        let syntax = read.startup_syntax().unwrap();
        assert_eq!(syntax, "SET TVARS=NAMES TNUMBERS=BOTH.");
        let mut session = statscraft_engine::session::Session::default();
        session.run(&syntax, None).unwrap();
        assert_eq!(session.setting("TVARS"), "NAMES");
        assert_eq!(session.setting("TNUMBERS"), "BOTH");
        assert!(session.settings_syntax().contains("TVARS='NAMES'"));
    }

    #[test]
    fn imputation_output_is_kept_and_set_when_a_session_starts() {
        let mut settings = Settings::default();
        settings.keep_mioutput("pooled observed");
        assert_eq!(settings.mioutput.as_deref(), Some("OBSERVED POOLED"));
        let text = settings.text();
        assert_eq!(text, "mioutput observed pooled\n");
        assert_eq!(Settings::parse(&text), settings);
        // none, or a word SET would refuse, is not kept
        assert_eq!(
            Settings::parse("mioutput\nmioutput pooled extra\n"),
            Settings::default()
        );
        settings.keep_mioutput("");
        assert_eq!(settings.mioutput, None);
        settings.keep_mioutput("IMPUTED DIAGNOSTICS");
        let syntax = settings.startup_syntax().unwrap();
        assert_eq!(syntax, "SET MIOUTPUT=[IMPUTED DIAGNOSTICS].");
        let mut session = statscraft_engine::session::Session::default();
        session.run(&syntax, None).unwrap();
        assert_eq!(session.setting("MIOUTPUT"), "[IMPUTED DIAGNOSTICS]");
    }
}
