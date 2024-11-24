mod app;
mod upload;
mod utils;

use app::ClaudeUploader;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([600.0, 600.0])
            .with_min_inner_size([400.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Claude.ai File Uploader",
        options,
        Box::new(|cc| Box::new(ClaudeUploader::new(cc))),
    )
}
