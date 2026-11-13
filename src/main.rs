// No console window when started by double-click.
#![windows_subsystem = "windows"]

mod analysis;
mod data;
mod gui;
mod rom;
mod save;
mod sprites;
mod update;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PKMN Lab")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("PKMN Lab", options, Box::new(|cc| Ok(Box::new(gui::App::new(cc)))))
}
