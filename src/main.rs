// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod schemas;
mod ui;
mod util;

use crate::ui::CheesePaperApp;

use directories::ProjectDirs;
use eframe::NativeOptions;

fn main() -> eframe::Result {
    env_logger::init();

    util::version::fetch_version();

    let project_dirs =
        ProjectDirs::from("", "", "cheese-paper").expect("home directories should always exist");

    let egui_data_path = project_dirs.data_dir().join("egui");

    let native_options = NativeOptions {
        persistence_path: Some(egui_data_path),
        ..Default::default()
    };

    eframe::run_native(
        "Cheese Paper",
        native_options,
        Box::new(|cc| Ok(Box::new(CheesePaperApp::new(cc, project_dirs)))),
    )
}
