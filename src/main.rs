#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod platform;
mod project;
mod terminal;

use std::env;
use std::path::PathBuf;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let root_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("superCLI")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "superCLI",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::SuperCliApp::new(cc, root_dir)))),
    )
}
