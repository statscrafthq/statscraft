//! Fonts come from the system: the fonts bundled with egui are not under MIT or Apache, so the
//! window loads common system fonts instead.

use eframe::egui::{FontData, FontDefinitions, FontFamily};
use std::path::PathBuf;
use std::sync::Arc;

fn candidates(mono: bool) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let variable = if mono {
        "STATSCRAFT_MONO_FONT"
    } else {
        "STATSCRAFT_FONT"
    };
    if let Some(path) = std::env::var_os(variable) {
        paths.push(PathBuf::from(path));
    }
    let windows = std::env::var_os("WINDIR").map_or(PathBuf::from(r"C:\Windows"), PathBuf::from);
    let names: &[&str] = if mono {
        &["consola.ttf", "cour.ttf"]
    } else {
        &["segoeui.ttf", "arial.ttf"]
    };
    paths.extend(names.iter().map(|name| windows.join("Fonts").join(name)));
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

fn load(mono: bool) -> Option<Vec<u8>> {
    candidates(mono)
        .into_iter()
        .find_map(|path| std::fs::read(path).ok())
}

/// A proportional and a monospace font, each the other's fallback.
pub fn definitions() -> Result<FontDefinitions, String> {
    let ui = load(false);
    let mono = load(true);
    let (ui, mono) = match (ui, mono) {
        (Some(ui), Some(mono)) => (ui, mono),
        (Some(one), None) | (None, Some(one)) => (one.clone(), one),
        (None, None) => {
            return Err("no usable system font found; set STATSCRAFT_FONT to a .ttf file".into());
        }
    };
    let mut fonts = FontDefinitions::empty();
    fonts
        .font_data
        .insert("ui".into(), Arc::new(FontData::from_owned(ui)));
    fonts
        .font_data
        .insert("mono".into(), Arc::new(FontData::from_owned(mono)));
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["ui".into(), "mono".into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["mono".into(), "ui".into()]);
    Ok(fonts)
}
