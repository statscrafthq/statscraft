// a release build on Windows opens no console window next to the application window
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod advanced_dialogs;
mod advanced_dialogs_view;
mod agreement_dialog;
mod agreement_dialog_view;
mod analyze_dialogs;
mod app;
mod bayes_dialog;
mod bayes_dialog_view;
mod brand;
mod catpca_dialog;
mod catpca_dialog_view;
mod chart_paint;
mod complete;
mod connect_window;
mod correspondence_dialog;
mod correspondence_dialog_view;
mod cox_dialog;
mod cox_dialog_view;
mod cs_model_dialog;
mod cs_model_dialog_view;
mod cs_sample_dialog;
mod cs_sample_dialog_view;
mod data_dialogs;
mod data_tools_dialog;
mod data_tools_dialog_view;
mod dialogs;
mod editor;
mod edits;
mod fonts;
mod forecast_more_dialog;
mod forecast_more_dialog_view;
mod genlin_dialog;
mod genlin_dialog_view;
mod graph_dialogs;
mod graph_dialogs_view;
mod grid;
mod grid_menu;
mod highlight;
mod imputation_dialog;
mod imputation_dialog_view;
mod loglinear_dialog;
mod loglinear_dialog_view;
mod mca_dialog;
mod mca_dialog_view;
mod mds_dialog;
mod mds_dialog_view;
mod model_dialogs;
mod model_dialogs_view;
mod model_more_dialog;
mod model_more_dialog_view;
mod mva_dialog;
mod mva_dialog_view;
mod nptests_dialog;
mod nptests_dialog_view;
mod office;
#[cfg(target_os = "macos")]
mod open_files;
mod output;
mod output_view;
mod plugins;
mod power_dialog;
mod power_dialog_view;
mod proportions_dialog;
mod proportions_dialog_view;
mod proximities_dialog;
mod proximities_dialog_view;
mod regression_more_dialog;
mod regression_more_dialog_view;
mod reports_dialog;
mod reports_dialog_view;
mod scaling_dialog;
mod scaling_dialog_view;
mod settings;
mod settings_window;
mod stats_dialogs;
mod stats_dialogs_view;
mod survival_dialog;
mod survival_dialog_view;
mod syntax_text;
mod syntax_window;
mod themes;
mod timeseries_dialog;
mod timeseries_dialog_view;
mod twostep_dialog;
mod twostep_dialog_view;
mod updates;
mod windows;

use eframe::egui;
use std::process::ExitCode;

fn main() -> ExitCode {
    let fonts = fonts::definitions();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let settings = settings::Settings::load();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Statscraft")
        .with_inner_size(settings.size.unwrap_or([1280.0, 820.0]))
        .with_min_inner_size([720.0, 480.0])
        .with_icon(icon());
    if let Some(position) = settings.position {
        viewport = viewport.with_position(position);
    }
    #[cfg(target_os = "macos")]
    open_files::listen();
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let result = eframe::run_native(
        // the app id (X11 class, Wayland app id): matches statscraft.desktop and its StartupWMClass
        "statscraft",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(fonts);
            #[cfg(target_os = "linux")]
            if let Some(theme) = gnome_theme() {
                // only where the window system reports no theme; a reported one still wins
                cc.egui_ctx.options_mut(|o| o.fallback_theme = theme);
            }
            #[cfg(target_os = "macos")]
            open_files::set_context(&cc.egui_ctx);
            Ok(Box::new(app::App::new(&args, &cc.egui_ctx, settings)))
        }),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("statscraft-gui: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The window icon. On Windows the executable also carries it, from build.rs.
fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../icons/statscraft-256.png"))
        .expect("the bundled icon is a valid PNG")
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_decodes() {
        assert_eq!(super::icon().width, 256);
    }
}

/// The light or dark choice of a GNOME desktop, for when the window system does not report it
/// (under X11, as in a remote desktop session); egui would otherwise fall back to dark.
// shortcut: read once at start, so a later change of the desktop theme needs a restart; other
// desktops keep egui's own fallback
#[cfg(target_os = "linux")]
fn gnome_theme() -> Option<egui::Theme> {
    let get = |key: &str| {
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", key])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    color_scheme(&get("color-scheme")?, get("gtk-theme").as_deref())
}

/// GNOME's color-scheme value as gsettings prints it, such as `'prefer-dark'`; `default` means no
/// preference, so the GTK theme's name decides (a dark one ends in `-dark`).
#[cfg(target_os = "linux")]
fn color_scheme(scheme: &str, gtk_theme: Option<&str>) -> Option<egui::Theme> {
    let unquote = |v: &str| v.trim().trim_matches('\'').to_ascii_lowercase();
    match unquote(scheme).as_str() {
        "prefer-dark" => Some(egui::Theme::Dark),
        "prefer-light" => Some(egui::Theme::Light),
        "default" => match gtk_theme.map(unquote) {
            Some(t) if t.ends_with("-dark") => Some(egui::Theme::Dark),
            _ => Some(egui::Theme::Light),
        },
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux"))]
mod theme_tests {
    #[test]
    fn gnome_color_scheme_picks_the_theme() {
        use super::{color_scheme, egui};
        let (dark, light) = (Some(egui::Theme::Dark), Some(egui::Theme::Light));
        assert_eq!(color_scheme("'prefer-dark'\n", None), dark);
        assert_eq!(color_scheme("'prefer-light'", Some("'Yaru-dark'")), light);
        assert_eq!(color_scheme("'default'\n", Some("'Yaru'\n")), light);
        assert_eq!(color_scheme("'default'", Some("'Adwaita-dark'")), dark);
        assert_eq!(color_scheme("'default'", None), light);
        assert_eq!(color_scheme("nonsense", None), None);
    }
}
