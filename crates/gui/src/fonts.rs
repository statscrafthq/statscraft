//! The window's fonts: Schibsted Grotesk for the interface and JetBrains Mono for syntax and
//! code, both bundled under the SIL Open Font License. System fonts follow them as fallbacks for
//! scripts the bundled fonts lack, such as Cyrillic in Schibsted Grotesk. `STATSCRAFT_FONT` and
//! `STATSCRAFT_MONO_FONT` name a .ttf file to use before the bundled ones.

use eframe::egui::{FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

/// Schibsted Grotesk with its tabular digits as the default ones (`fonts/tabular.py`), so that
/// numbers in the grids and the output line up.
const UI: &[u8] = include_bytes!("../fonts/SchibstedGrotesk.ttf");
/// JetBrains Mono without ligatures, so `<=` and `~=` in syntax show as typed.
const MONO: &[u8] = include_bytes!("../fonts/JetBrainsMonoNL-Regular.ttf");

/// The fonts' copyright and license, which the license asks to ship with them; Help, About shows it.
pub const NOTICES: &str = concat!(
    "Fonts: Schibsted Grotesk (its digits made tabular) and JetBrains Mono, each under the SIL \
     Open Font License, Version 1.1.\n\n",
    include_str!("../fonts/OFL-SchibstedGrotesk.txt"),
    "\n",
    include_str!("../fonts/OFL-JetBrainsMono.txt"),
);

fn system_candidates(mono: bool) -> Vec<PathBuf> {
    let windows = std::env::var_os("WINDIR").map_or(PathBuf::from(r"C:\Windows"), PathBuf::from);
    let names: &[&str] = if mono {
        &["consola.ttf", "cour.ttf"]
    } else {
        &["segoeui.ttf", "arial.ttf"]
    };
    let mut paths: Vec<PathBuf> = names
        .iter()
        .map(|name| windows.join("Fonts").join(name))
        .collect();
    let unix: &[&str] = if mono {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Courier New.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial.ttf",
        ]
    };
    paths.extend(unix.iter().map(PathBuf::from));
    paths
}

fn read(paths: impl IntoIterator<Item = PathBuf>) -> Option<Vec<u8>> {
    paths.into_iter().find_map(|path| std::fs::read(path).ok())
}

/// The proportional and the monospace family: the font a variable names, the bundled font, a
/// system font, and the other family's bundled font last.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    let mut add = |name: &str, data: FontData| {
        fonts.font_data.insert(name.into(), Arc::new(data));
        name.to_string()
    };
    let mut proportional = Vec::new();
    let mut monospace = Vec::new();
    for (mono, variable, bundled, family) in [
        (false, "STATSCRAFT_FONT", UI, &mut proportional),
        (true, "STATSCRAFT_MONO_FONT", MONO, &mut monospace),
    ] {
        let tag = if mono { "mono" } else { "ui" };
        if let Some(own) = std::env::var_os(variable).and_then(|p| read([PathBuf::from(p)])) {
            family.push(add(&format!("{tag}-own"), FontData::from_owned(own)));
        }
        family.push(add(tag, FontData::from_static(bundled)));
        if let Some(system) = read(system_candidates(mono)) {
            family.push(add(&format!("{tag}-system"), FontData::from_owned(system)));
        }
    }
    proportional.push("mono".into());
    monospace.push("ui".into());
    fonts
        .families
        .insert(FontFamily::Proportional, proportional);
    fonts.families.insert(FontFamily::Monospace, monospace);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_fonts_come_first_and_carry_their_license() {
        let fonts = definitions();
        let first = |family| {
            fonts.families[&family]
                .iter()
                .find(|n| !n.ends_with("-own"))
        };
        assert_eq!(first(FontFamily::Proportional).unwrap(), "ui");
        assert_eq!(first(FontFamily::Monospace).unwrap(), "mono");
        assert_eq!(
            fonts.families[&FontFamily::Proportional].last().unwrap(),
            "mono"
        );
        assert!(NOTICES.matches("SIL OPEN FONT LICENSE Version 1.1").count() == 2);
    }
}
