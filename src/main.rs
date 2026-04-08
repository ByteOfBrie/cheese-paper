// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod schemas;
mod ui;
mod util;

use crate::ui::CheesePaperApp;

use directories::ProjectDirs;
use eframe::NativeOptions;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode, colored_opt_format, opt_format};

fn main() -> eframe::Result {
    let project_dirs = match ProjectDirs::from("", "", "cheese-paper") {
        Some(dirs) => dirs,
        None => {
            eprintln!(
                "Could not load home directories, which should always exist, we cannot log yet, giving up"
            );
            panic!("home directories should always exist");
        }
    };

    let egui_data_path = project_dirs.data_dir().join("egui");

    match Logger::try_with_env_or_str("info,cheese_paper=debug") {
        Ok(logger) => {
            if let Err(err) = logger
                .log_to_file(FileSpec::default().directory(project_dirs.data_dir().join("logs")))
                .append()
                .duplicate_to_stdout(Duplicate::Debug)
                .rotate(
                    flexi_logger::Criterion::Size(100_000),
                    flexi_logger::Naming::TimestampsDirect,
                    flexi_logger::Cleanup::KeepForDays(14),
                )
                .write_mode(WriteMode::BufferAndFlush)
                .format_for_files(opt_format)
                .format_for_stdout(colored_opt_format)
                .start()
            {
                eprintln!("Could not start logger: {err}");
            }
        }
        Err(err) => {
            eprintln!("Could not create logger: {err}");
        }
    };

    let icon_data_res =
        eframe::icon_data::from_png_bytes(include_bytes!("../resources/cheese-paper-icon.png"));

    let viewport = if let Ok(icon_data) = icon_data_res {
        egui::ViewportBuilder::default()
            .with_icon(icon_data)
            .with_app_id("cheese-paper")
    } else {
        // if you clone without setting up git lfs, we won't have a valid icon file.
        // we just proceed without it for now
        log::warn!("Could not load icon data: did this repo have git lfs set up?");
        egui::ViewportBuilder::default().with_app_id("cheese-paper")
    };

    let native_options = NativeOptions {
        persistence_path: Some(egui_data_path),
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Cheese Paper",
        native_options,
        Box::new(|cc| Ok(Box::new(CheesePaperApp::new(cc, project_dirs)))),
    )
}
