//! Design templates of a plugin's settings, and the file that carries them from the window to
//! a headless run (docs/plugins.md).
//!
//! A template is a named set of values for the settings of one plugin that are not kept per
//! data file. In a run it lies over the data file's attributes: its values win. The file is the
//! window's own settings lines, so a template can be copied between them: `plugin-template
//! PLUGIN<tab>NAME<tab>ATTRIBUTE<tab>VALUE` (or without the last two for a template without
//! values) and `preset ATTRIBUTE<tab>NAME<tab>VALUE`. Blank lines and lines starting with `*`
//! are skipped; any other line is an error, so a typo does not silently drop a value.

use crate::plugin::Description;

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

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
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
        // an older template without values names no plugin, so it belongs to none
        !self.values.is_empty() && self.values.iter().all(|(a, _)| mine(a))
    }
}

/// Adds one template line to `templates`, after its plugin name: `NAME<tab>ATTRIBUTE<tab>VALUE`,
/// or `NAME` for a template without values. A line without a name adds nothing.
pub fn read_template(templates: &mut Vec<Template>, plugin: &str, rest: &str) {
    let parts: Vec<&str> = rest.split('\t').collect();
    if parts[0].is_empty() {
        return;
    }
    let found = templates
        .iter()
        .position(|t| t.plugin == plugin && t.name == parts[0]);
    let at = found.unwrap_or_else(|| {
        templates.push(Template::new(plugin, parts[0], &[]));
        templates.len() - 1
    });
    if let [_, attribute, value] = parts[..]
        && !attribute.is_empty()
    {
        let pair = (attribute.to_string(), value.to_string());
        templates[at].values.push(pair);
    }
}

/// The attributes with the template laid over them: its values win for every setting not kept
/// per data file (with the setting's preset name); a setting the template lacks keeps what the
/// data or this computer has, else the plugin's default.
pub fn with_template(
    description: &Description,
    mut all: Vec<(String, Vec<String>)>,
    template: Option<&Template>,
) -> Vec<(String, Vec<String>)> {
    let Some(template) = template else {
        return all;
    };
    for s in description.settings.iter().filter(|s| s.in_templates()) {
        let preset = s.presets.then(|| description.preset_attribute(&s.key));
        for name in [Some(description.attribute(&s.key)), preset]
            .into_iter()
            .flatten()
        {
            let found = template
                .values
                .iter()
                .find(|(a, _)| a.eq_ignore_ascii_case(&name));
            if let Some((_, value)) = found {
                all.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
                all.push((name, vec![value.clone()]));
            }
        }
    }
    all
}

/// What a template file holds: templates, and the saved values (presets) of settings, each as
/// (attribute, name, value).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemplateFile {
    pub templates: Vec<Template>,
    pub presets: Vec<(String, String, String)>,
}

impl TemplateFile {
    /// Reads the file's text; the error names the line.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut file = TemplateFile::default();
        for (i, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || line.starts_with('*') {
                continue;
            }
            let bad = |what: &str| format!("line {}: {what}", i + 1);
            let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "plugin-template" => {
                    let Some((plugin, rest)) = rest.split_once('\t') else {
                        return Err(bad("plugin-template needs the plugin, a tab and the name"));
                    };
                    if rest.split('\t').next().is_none_or(str::is_empty) {
                        return Err(bad("a template needs a name"));
                    }
                    read_template(&mut file.templates, plugin, rest);
                }
                "preset" => match rest.split('\t').collect::<Vec<_>>()[..] {
                    [attribute, name, value] if !attribute.is_empty() && !name.is_empty() => {
                        let clean = |s: &str| s.trim().to_string();
                        file.presets
                            .push((attribute.to_string(), clean(name), clean(value)));
                    }
                    _ => return Err(bad("preset needs the attribute, the name and the value")),
                },
                _ => return Err(bad("expected plugin-template or preset")),
            }
        }
        Ok(file)
    }

    /// The text of the file.
    pub fn text(&self) -> String {
        let mut text = String::from(
            "* A Statscraft plugin template: the lines of the window's settings file.\n",
        );
        for p in &self.presets {
            text.push_str(&format!("preset {}\t{}\t{}\n", p.0, p.1, p.2));
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
        text
    }

    /// The one template a headless run applies: the file must hold exactly one.
    pub fn single(&self) -> Result<&Template, String> {
        match &self.templates[..] {
            [one] => Ok(one),
            [] => Err("the file holds no template".into()),
            many => Err(format!(
                "the file holds {} templates; give a file with one",
                many.len()
            )),
        }
    }
}

/// Reads the template files of a run and finds the plugin each is for. A file that cannot be
/// read, holds not exactly one template, or whose plugin is none of `programs` is an error
/// naming the file; two for one plugin is an error too. Returns one template per file.
pub fn load(
    files: &[std::path::PathBuf],
    descriptions: &[Description],
) -> Result<Vec<Template>, String> {
    let mut all: Vec<Template> = Vec::new();
    for file in files {
        let name = file.display();
        let text = std::fs::read_to_string(file).map_err(|e| format!("{name}: {e}"))?;
        let parsed = TemplateFile::parse(&text).map_err(|e| format!("{name}: {e}"))?;
        let template = parsed.single().map_err(|e| format!("{name}: {e}"))?;
        let Some(plugin) = descriptions.iter().find(|d| template.belongs_to(d)) else {
            return Err(format!(
                "{name}: template {} is for the plugin {}, which no --plugin names",
                template.name,
                if template.plugin.is_empty() {
                    "of its attributes"
                } else {
                    &template.plugin
                }
            ));
        };
        if all.iter().any(|t| t.belongs_to(plugin)) {
            return Err(format!(
                "{name}: a second template for the plugin {}",
                plugin.name
            ));
        }
        all.push(template.clone());
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Kind, Setting};

    fn setting(key: &str, presets: bool, per_file: bool) -> Setting {
        Setting {
            global: false,
            choices: Vec::new(),
            key: key.into(),
            label: key.into(),
            kind: Kind::Text,
            default: "".into(),
            presets,
            per_file,
            group: None,
        }
    }

    fn house() -> Description {
        Description {
            name: "House".into(),
            version: "1".into(),
            protocol: 1,
            settings: vec![setting("high", true, false), setting("client", false, true)],
            updates: false,
        }
    }

    #[test]
    fn a_file_keeps_templates_and_presets_through_its_text() {
        let file = TemplateFile {
            templates: vec![
                Template::new(
                    "House",
                    "Client A",
                    &[("plugin_house_high".into(), "#ed7d31".into())],
                ),
                Template::new("House", "Empty", &[]),
            ],
            presets: vec![("plugin_house_high".into(), "Warm".into(), "#ed7d31".into())],
        };
        assert_eq!(TemplateFile::parse(&file.text()).unwrap(), file);
        // a text from the window's settings file, with a byte order mark and a comment
        let window = "\u{feff}* comment\r\n\r\nplugin-template House\tClient A\tplugin_house_high\t#ed7d31\r\n";
        let parsed = TemplateFile::parse(window).unwrap();
        assert_eq!(parsed.templates[0].values[0].1, "#ed7d31");
        assert!(parsed.single().is_ok());
    }

    #[test]
    fn a_line_that_is_neither_a_template_nor_a_preset_is_an_error_with_its_number() {
        let e = TemplateFile::parse("* a\nplugin-templat House\tx\n").unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
        assert!(TemplateFile::parse("plugin-template House\t\n").is_err());
        assert!(TemplateFile::parse("preset plugin_house_high\tonly a name\n").is_err());
    }

    #[test]
    fn a_run_applies_exactly_one_template_per_plugin_and_names_the_file_that_fails() {
        let dir = std::env::temp_dir().join(format!("statscraft-template-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, text: &str| {
            let path = dir.join(name);
            std::fs::write(&path, text).unwrap();
            path
        };
        let one = write(
            "one.txt",
            "plugin-template House\tA\tplugin_house_high\t#111111\n",
        );
        let two = write(
            "two.txt",
            "plugin-template House\tA\nplugin-template House\tB\n",
        );
        let none = write("none.txt", "* nothing\n");
        let other = write("other.txt", "plugin-template Heat\tA\tplugin_heat_low\t1\n");
        let house = [house()];
        let found = load(std::slice::from_ref(&one), &house).unwrap();
        assert_eq!(found[0].name, "A");
        let error = |files: &[std::path::PathBuf]| load(files, &house).unwrap_err();
        assert!(error(&[two]).contains("holds 2 templates"));
        assert!(error(&[none]).contains("holds no template"));
        assert!(error(&[other]).contains("is for the plugin Heat"));
        assert!(error(&[one.clone(), one]).contains("a second template for the plugin House"));
        assert!(error(&[dir.join("missing.txt")]).contains("missing.txt"));
        // the template wins for a shared setting, never for one kept per data file
        let template = Template::new(
            "House",
            "A",
            &[
                ("plugin_house_high".into(), "#111111".into()),
                ("plugin_house_client".into(), "other".into()),
            ],
        );
        let data = vec![
            ("plugin_house_high".to_string(), vec!["#999999".to_string()]),
            ("plugin_house_client".to_string(), vec!["mine".to_string()]),
        ];
        let all = with_template(&house[0], data, Some(&template));
        let value = |name: &str| all.iter().find(|(n, _)| n == name).unwrap().1[0].clone();
        assert_eq!(value("plugin_house_high"), "#111111");
        assert_eq!(value("plugin_house_client"), "mine");
        let _ = std::fs::remove_dir_all(dir);
    }
}
