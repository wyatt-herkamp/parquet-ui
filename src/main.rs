mod app;
mod format;
mod parquet_io;
mod views;
mod wrangle;

use app::App;
use tracing_subscriber::EnvFilter;

fn main() -> iced::Result {
    // Default filter quiets DataFusion/arrow internals; override with RUST_LOG.
    let default_filter = "parquet_ui=info,warn";
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    tracing::info!("starting parquet-ui");

    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .run()
}
