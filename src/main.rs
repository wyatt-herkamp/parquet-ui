mod app;
mod format;
mod parquet_io;
mod theme;
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

    fn theme_for(_: &App) -> iced::Theme {
        theme::instrument_theme()
    }

    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(theme_for)
        .font(theme::GEIST_REGULAR_BYTES)
        .font(theme::GEIST_MEDIUM_BYTES)
        .font(theme::GEIST_SEMIBOLD_BYTES)
        .font(theme::JETBRAINS_MONO_REGULAR_BYTES)
        .font(theme::JETBRAINS_MONO_MEDIUM_BYTES)
        .default_font(theme::FONT_UI)
        .run()
}
