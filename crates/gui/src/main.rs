// a release build on Windows opens no console window next to the application window
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dialogs;
mod editor;
mod edits;
mod fonts;
mod grid;
mod output;
mod syntax_text;

use eframe::egui;
use std::process::ExitCode;

fn main() -> ExitCode {
    let fonts = match fonts::definitions() {
        Ok(fonts) => fonts,
        Err(e) => {
            eprintln!("statscraft-gui: {e}");
            return ExitCode::FAILURE;
        }
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("StatsCraft")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "StatsCraft",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(app::App::new(&args, &cc.egui_ctx)))
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
