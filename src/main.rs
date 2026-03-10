// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod schemas;
mod ui;
mod util;

use crate::ui::CheesePaperApp;

use directories::ProjectDirs;
use eframe::NativeOptions;
use log::LevelFilter;

fn main() -> eframe::Result {
    env_logger::Builder::new()
        .filter_module("tracing::span", LevelFilter::Warn)
        .filter_module("winit::window", LevelFilter::Warn)
        .parse_default_env()
        .init();

    let project_dirs =
        ProjectDirs::from("", "", "cheese-paper").expect("home directories should always exist");

    let egui_data_path = project_dirs.data_dir().join("egui");

    let icon_data =
        eframe::icon_data::from_png_bytes(include_bytes!("../resources/cheese-paper-icon.png"))
            .unwrap();

    let native_options = NativeOptions {
        persistence_path: Some(egui_data_path),
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon_data)
            .with_app_id("cheese-paper"),
        ..Default::default()
    };

    eframe::run_native(
        "Cheese Paper",
        native_options,
        Box::new(|cc| Ok(Box::new(CheesePaperApp::new(cc, project_dirs)))),
    )
}
