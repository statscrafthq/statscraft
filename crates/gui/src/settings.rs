//! What the window remembers between starts: its size and position, the folder of the last
//! file dialog, the recent files and the settings of Edit > Settings. Kept in the user's config
//! folder as lines of text. How tables show variables and values (SET TVARS and TNUMBERS) and
//! the parts of multiply imputed output (SET MIOUTPUT) are kept here too, and a new session of
//! the window starts by running that SET.

use crate::output::{Part, Show};
use statscraft_engine::plugin::Description;
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
    /// The plugin programs that decorate tables (docs/plugins.md), in the order they run.
    pub plugins: Vec<Plugin>,
    /// Excel exports leave out the plugins' comments.
    pub excel_comments_off: bool,
    /// Saved values of plugin settings, for every data file.
    pub presets: Vec<Preset>,
    /// The plugin settings last chosen for each data file, by its path: they hold when the
    /// file is read again (GET FILE in a syntax) before its new data file attributes were saved.
    pub file_values: Vec<FileValue>,
    /// Design templates of the plugins' settings, and the one in use for each plugin as
    /// (plugin name, template name); the plugin name is empty in a line from before there were
    /// several plugins.
    pub templates: Vec<Template>,
    pub active_templates: Vec<(String, String)>,
}

/// A plugin program, and whether it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub path: PathBuf,
    pub on: bool,
}

/// A named set of one plugin's setting values (every setting not kept per data file), as
/// (attribute, value) pairs; one template per client's look, say.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// The name the plugin describes itself with; empty in a template saved before there were
    /// several plugins.
    pub plugin: String,
    pub name: String,
    pub values: Vec<(String, String)>,
}

impl Template {
    /// A template from the dialog: tabs and line breaks become spaces, so each value fits its
    /// line of the settings file.
    pub fn new(plugin: &str, name: &str, values: &[(String, String)]) -> Self {
        let values = values.iter().map(|(a, v)| (clean(a), clean(v)));
        Template {
            plugin: clean(plugin),
            name: clean(name).trim().to_string(),
            values: values.collect(),
        }
    }

    /// Whether the template holds settings of the plugin: it was saved for it, or it has no
    /// plugin name and every attribute it holds is one of the plugin's (`plugin_<name>_...`).
    pub fn belongs_to(&self, plugin: &Description) -> bool {
        if !self.plugin.is_empty() {
            return self.plugin == plugin.name;
        }
        let prefix = plugin.attribute("");
        let mine = |a: &str| a.to_ascii_lowercase().starts_with(&prefix);
        self.values.iter().all(|(a, _)| mine(a))
    }
}

/// The template in use for a plugin: the one chosen for it, else one chosen before there were
/// several plugins that holds its settings.
pub fn active_template<'a>(
    templates: &'a [Template],
    active: &[(String, String)],
    plugin: &Description,
) -> Option<&'a Template> {
    let chosen = |exact: bool| {
        active.iter().find_map(|(p, name)| {
            let fits = if exact {
                *p == plugin.name
            } else {
                p.is_empty()
            };
            let found = templates
                .iter()
                .find(|t| &t.name == name && t.belongs_to(plugin));
            found.filter(|_| fits)
        })
    };
    chosen(true).or_else(|| chosen(false))
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
}

/// A window size worth opening with again: at least 300 by 200 points and not absurdly large. A
/// window that was never shown (or minimized) reports tiny sizes such as 0 by 0 or 16 by 39.
pub fn usable_size([w, h]: [f32; 2]) -> bool {
    (300.0..=20_000.0).contains(&w) && (200.0..=20_000.0).contains(&h)
}

/// A window position worth opening at again; a minimized window on Windows reports -32000.
pub fn usable_position([x, y]: [f32; 2]) -> bool {
    (-10_000.0..=20_000.0).contains(&x) && (-10_000.0..=20_000.0).contains(&y)
}

impl Settings {
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
    /// `export hidden`, one line per plugin in order (`plugin PATH`, or `plugin-off PATH` for
    /// one turned off), `excel-comments off`,
    /// `preset ATTRIBUTE<tab>NAME<tab>VALUE`, `plugin-value ATTRIBUTE<tab>VALUE<tab>FILE`,
    /// `plugin-template PLUGIN<tab>NAME<tab>ATTRIBUTE<tab>VALUE` (or `plugin-template
    /// PLUGIN<tab>NAME` for one without values) and `plugin-template-active PLUGIN<tab>NAME`;
    /// anything else is skipped. The older lines `plugin-off` alone (the one plugin is off),
    /// `template NAME<tab>ATTRIBUTE<tab>VALUE`, `template NAME` and `template-active NAME` are
    /// read too.
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        let mut old_off = false;
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
                // a size or position no window can usefully have is forgotten
                "size" => settings.size = pair(rest).filter(|&s| usable_size(s)),
                "position" => settings.position = pair(rest).filter(|&p| usable_position(p)),
                "syntax-size" => settings.syntax_size = pair(rest).filter(|&s| usable_size(s)),
                "syntax-position" => {
                    settings.syntax_position = pair(rest).filter(|&p| usable_position(p))
                }
                "output-size" => settings.output_size = pair(rest).filter(|&s| usable_size(s)),
                "output-position" => {
                    settings.output_position = pair(rest).filter(|&p| usable_position(p))
                }
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
                "plugin" | "plugin-off" if !rest.is_empty() => settings.plugins.push(Plugin {
                    path: PathBuf::from(rest),
                    on: key == "plugin",
                }),
                // the older form: one `plugin PATH` line and this one turning it off
                "plugin-off" => old_off = true,
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
                "plugin-template" => {
                    if let Some((plugin, rest)) = rest.split_once('\t') {
                        settings.read_template(plugin, rest);
                    }
                }
                // the older form, without the plugin's name
                "template" => settings.read_template("", rest),
                "plugin-template-active" => {
                    if let Some((plugin, name)) = rest.split_once('\t')
                        && !name.is_empty()
                    {
                        let active = (plugin.to_string(), name.to_string());
                        settings.active_templates.push(active);
                    }
                }
                "template-active" if !rest.is_empty() => {
                    settings.active_templates.push((String::new(), rest.into()))
                }
                _ => {}
            }
        }
        if old_off {
            settings.plugins.iter_mut().for_each(|p| p.on = false);
        }
        settings
    }

    /// A template line after its plugin name: `NAME<tab>ATTRIBUTE<tab>VALUE`, or `NAME` for a
    /// template without values.
    fn read_template(&mut self, plugin: &str, rest: &str) {
        let parts: Vec<&str> = rest.split('\t').collect();
        if parts[0].is_empty() {
            return;
        }
        let found = self
            .templates
            .iter()
            .position(|t| t.plugin == plugin && t.name == parts[0]);
        let at = found.unwrap_or_else(|| {
            self.templates.push(Template::new(plugin, parts[0], &[]));
            self.templates.len() - 1
        });
        if let [_, attribute, value] = parts[..]
            && !attribute.is_empty()
        {
            let pair = (attribute.to_string(), value.to_string());
            self.templates[at].values.push(pair);
        }
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
        for p in &self.plugins {
            let key = if p.on { "plugin" } else { "plugin-off" };
            text.push_str(&format!("{key} {}\n", p.path.display()));
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
            let head = format!("plugin-template {}\t{}", t.plugin, t.name);
            if t.values.is_empty() {
                text.push_str(&format!("{head}\n"));
            }
            for (attribute, value) in &t.values {
                text.push_str(&format!("{head}\t{attribute}\t{value}\n"));
            }
        }
        for (plugin, name) in &self.active_templates {
            text.push_str(&format!("plugin-template-active {plugin}\t{name}\n"));
        }
        text
    }

    /// The design template in use for a plugin, if any.
    pub fn active_template(&self, plugin: &Description) -> Option<&Template> {
        active_template(&self.templates, &self.active_templates, plugin)
    }

    /// The templates of a plugin (`Template::belongs_to`).
    pub fn templates_of(&self, plugin: &Description) -> Vec<Template> {
        let mine = self.templates.iter().filter(|t| t.belongs_to(plugin));
        mine.cloned().collect()
    }

    /// Puts a plugin's templates, as its Plugin Settings dialog left them, in place of those it
    /// had, each saved for the plugin, and makes `chosen` the plugin's active template.
    pub fn set_templates(
        &mut self,
        plugin: &Description,
        mine: Vec<Template>,
        chosen: Option<String>,
    ) {
        self.templates.retain(|t| !t.belongs_to(plugin));
        let templates = &self.templates;
        // an older choice without a plugin name goes when its template went
        let left = |(p, name): &(String, String)| {
            *p != plugin.name && (!p.is_empty() || templates.iter().any(|t| &t.name == name))
        };
        self.active_templates.retain(left);
        let name = &plugin.name;
        let mine = mine
            .into_iter()
            .map(|t| Template::new(name, &t.name, &t.values));
        self.templates.extend(mine);
        if let Some(chosen) = chosen {
            self.active_templates.push((name.clone(), chosen));
        }
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
            plugins: vec![
                Plugin {
                    path: PathBuf::from("/opt/plugins/heat map"),
                    on: false,
                },
                Plugin {
                    path: PathBuf::from("/opt/plugins/house"),
                    on: true,
                },
            ],
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
                    "House",
                    "Client A",
                    &[
                        ("plugin_house_high".into(), "#ed7d31".into()),
                        ("plugin_house_head".into(), "{\"bold\":true}\tx".into()),
                    ],
                ),
                Template::new("House", "Empty one", &[]),
                Template::new(
                    "Heat Map",
                    "Client A",
                    &[("plugin_heat_map_high".into(), "#000000".into())],
                ),
                // from before there were several plugins
                Template::new("", "Old", &[("plugin_house_high".into(), "#111111".into())]),
            ],
            active_templates: vec![
                ("House".into(), "Client A".into()),
                ("Heat Map".into(), "Client A".into()),
            ],
        };
        assert_eq!(settings.templates[0].values[1].1, "{\"bold\":true} x");
        let house = described("House");
        assert_eq!(
            settings.active_template(&house),
            Some(&settings.templates[0])
        );
        assert_eq!(
            settings.active_template(&described("Heat Map")),
            Some(&settings.templates[2])
        );
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
        // a window that was never shown, or a minimized one, is not remembered
        let tiny = Settings::parse(
            "size 0 0\nposition 1 1\nsyntax-size 16 39\noutput-position -32000 -32000\n",
        );
        assert_eq!(tiny.size, None);
        assert_eq!(tiny.syntax_size, None);
        assert_eq!(tiny.position, Some([1.0, 1.0]));
        assert_eq!(tiny.output_position, None);
        let fine = Settings::parse("size 1280 820\nposition 40 30\n");
        assert_eq!(
            (fine.size, fine.position),
            (Some([1280.0, 820.0]), Some([40.0, 30.0]))
        );
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

    /// A plugin's description with a name and no settings.
    fn described(name: &str) -> Description {
        Description {
            name: name.into(),
            version: "1".into(),
            protocol: 1,
            settings: Vec::new(),
        }
    }

    /// A file from before there were several plugins: one plugin turned off by a line of its
    /// own, templates without a plugin name, and the active one by name only. Each template
    /// belongs to the plugin whose attributes it holds; the new form is written.
    #[test]
    fn the_older_plugin_lines_still_read() {
        let old = "plugin /opt/house\nplugin-off\n\
                   template Client A\tplugin_house_high\t#ed7d31\n\
                   template Client A\tplugin_house_limit\t9\n\
                   template Heat\tplugin_heat_map_low\t#ffffff\n\
                   template-active Client A\n";
        let mut settings = Settings::parse(old);
        let plugin = |path: &str, on| Plugin {
            path: PathBuf::from(path),
            on,
        };
        assert_eq!(settings.plugins, [plugin("/opt/house", false)]);
        let (house, heat) = (described("House"), described("Heat Map"));
        let names = |d: &Description| -> Vec<String> {
            settings
                .templates_of(d)
                .into_iter()
                .map(|t| t.name)
                .collect()
        };
        assert_eq!(names(&house), ["Client A"]);
        assert_eq!(names(&heat), ["Heat"]);
        assert_eq!(settings.active_template(&house).unwrap().values.len(), 2);
        assert_eq!(settings.active_template(&heat), None);
        let text = settings.text();
        assert!(text.starts_with("plugin-off /opt/house\n"), "{text}");
        assert!(text.contains("plugin-template \tClient A\tplugin_house_limit\t9\n"));
        assert!(text.contains("plugin-template-active \tClient A\n"));
        assert_eq!(Settings::parse(&text), settings);
        // OK in the house plugin's dialog saves its templates under its name
        let mine = settings.templates_of(&house);
        settings.set_templates(&house, mine, Some("Client A".into()));
        assert_eq!(settings.templates[0].plugin, "");
        assert_eq!(settings.templates[1].plugin, "House");
        assert_eq!(
            settings.active_templates,
            [("House".to_string(), "Client A".to_string())]
        );
        assert_eq!(settings.active_template(&house).unwrap().plugin, "House");
        // turning a template off for one plugin leaves another's choice alone
        settings.set_templates(&heat, settings.templates_of(&heat), Some("Heat".into()));
        settings.set_templates(&house, settings.templates_of(&house), None);
        assert_eq!(settings.active_template(&house), None);
        assert_eq!(settings.active_template(&heat).unwrap().name, "Heat");
        assert_eq!(Settings::parse(&settings.text()), settings);
        // several plugins in order, each on or off
        let text = "plugin /a\nplugin-off /b\nplugin /c\n";
        let read = Settings::parse(text);
        let expected = [plugin("/a", true), plugin("/b", false), plugin("/c", true)];
        assert_eq!(read.plugins, expected);
        assert_eq!(read.text(), text);
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
