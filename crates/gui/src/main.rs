// a release build on Windows opens no console window next to the application window
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod advanced_dialogs;
mod advanced_dialogs_view;
mod analyze_dialogs;
mod app;
mod brand;
mod chart_paint;
mod complete;
mod connect_window;
mod correspondence_dialog;
mod correspondence_dialog_view;
mod data_dialogs;
mod dialogs;
mod editor;
mod edits;
mod fonts;
mod genlin_dialog;
mod genlin_dialog_view;
mod graph_dialogs;
mod graph_dialogs_view;
mod grid;
mod grid_menu;
mod highlight;
mod model_dialogs;
mod model_dialogs_view;
mod office;
#[cfg(target_os = "macos")]
mod open_files;
mod output;
mod output_view;
mod settings;
mod settings_window;
mod stats_dialogs;
mod stats_dialogs_view;
mod syntax_text;
mod syntax_window;
mod themes;
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
