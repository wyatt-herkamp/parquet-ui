mod app;
mod format;
mod parquet_io;
mod views;

use app::App;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .run()
}
