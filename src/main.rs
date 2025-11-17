mod gui;
mod hashers;
mod file_ops;
mod models;
mod storage;
mod utils;

use iced::{window, Size};

fn main() -> iced::Result {
    iced::application(
        "VeriFILE - File Verifier",
        gui::VeriFileApp::update,
        gui::VeriFileApp::view,
    )
    .window(window::Settings {
        size: Size::new(1200.0, 760.0),
        resizable: true,
        ..window::Settings::default()
    })
    .run_with(gui::VeriFileApp::new)
}
