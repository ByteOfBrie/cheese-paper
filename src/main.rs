// hide console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod schemas;
mod ui;
mod util;

use std::sync::Arc;

use crate::ui::CheesePaperApp;

use directories::ProjectDirs;
use eframe::{NativeOptions, egui_wgpu::WgpuSetup, wgpu};
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

    match Logger::try_with_env_or_str("warn,cheese_paper=info") {
        Ok(logger) => {
            if let Err(err) = logger
                .log_to_file(FileSpec::default().directory(project_dirs.data_dir().join("logs")))
                .append()
                .duplicate_to_stdout(Duplicate::Debug)
                .rotate(
                    flexi_logger::Criterion::Size(5_000_000),
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
            .with_app_id("gay.brie.CheesePaper")
    } else {
        // if you clone without setting up git lfs, we won't have a valid icon file.
        // we just proceed without it for now
        log::warn!("Could not load icon data: did this repo have git lfs set up?");
        egui::ViewportBuilder::default().with_app_id("gay.brie.CheesePaper")
    };

    let mut native_options = NativeOptions {
        persistence_path: Some(egui_data_path),
        viewport,
        ..Default::default()
    };

    // To fix https://codeberg.org/ByteOfBrie/cheese-paper/issues/318, we need to override
    // native_options.wgpu_options.wgpu_setup.device_descriptor.required_limits.max_texture_dimension_3d
    //
    // Since Cheese Paper doesn't use *any* 3d rendering, we can simply set this to 0, but it's
    // exceptionally annoying to get to that value. This is a copy of the relevant value that gets
    // set in egui-wgpu, but with `max_texture_dimension_3d` set to 0 (and a different label)
    //
    // https://github.com/emilk/egui/blob/0.35.0/crates/egui-wgpu/src/setup.rs#L256-L273
    if let WgpuSetup::CreateNew(wgpu_setup_creator) = &mut native_options.wgpu_options.wgpu_setup {
        wgpu_setup_creator.device_descriptor = Arc::new(|adapter| {
            let base_limits = if adapter.get_info().backend == wgpu::Backend::Gl {
                wgpu::Limits::downlevel_webgl2_defaults()
            } else {
                wgpu::Limits::default()
            };

            wgpu::DeviceDescriptor {
                label: Some("cheese-paper egui wgpu device"),
                required_limits: wgpu::Limits {
                    // When using a depth buffer, we have to be able to create a texture
                    // large enough for the entire surface, and we want to support 4k+ displays.
                    max_texture_dimension_2d: 8192,
                    max_texture_dimension_3d: 0,
                    ..base_limits
                },
                ..Default::default()
            }
        })
    }

    eframe::run_native(
        "Cheese Paper",
        native_options,
        Box::new(|cc| Ok(Box::new(CheesePaperApp::new(cc, project_dirs)))),
    )
}
