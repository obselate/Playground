//! mile-marker GUI entry point.
//!
//! Almost everything interesting lives in [`mile_marker::app::MileMarkerApp`];
//! this file just opens an `eframe` window and hands control over.

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("mile-marker"),
        ..Default::default()
    };
    eframe::run_native(
        "mile-marker",
        options,
        Box::new(|cc| Ok(Box::new(mile_marker::app::MileMarkerApp::new(cc)))),
    )
}
