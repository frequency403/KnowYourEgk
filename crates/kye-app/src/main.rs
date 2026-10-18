#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! KnowYourEgk: reads the insurance master data (VSD) of a German eGK G2.1,
//! inserted (contact) or placed on the reader (contactless, PACE with CAN).

mod app;
mod export;
mod i18n;
mod icons;
mod theme;
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
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "KnowYourEgk",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)?))),
    )
}
