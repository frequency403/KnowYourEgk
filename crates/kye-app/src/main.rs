#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! KnowYourEgk: reads the insurance master data (VSD) of a German eGK G2.1,
//! inserted (contact) or placed on the reader (contactless, PACE with CAN).

mod app;
mod worker;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("KnowYourEgk")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "KnowYourEgk",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)?))),
    )
}
