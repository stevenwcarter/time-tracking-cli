pub mod config;
pub mod display;
pub mod editor;
pub mod file_utils;
pub mod time_utils;

#[cfg(feature = "webapp")]
pub mod context;
#[cfg(feature = "webapp")]
pub mod graphql;
#[cfg(feature = "webapp")]
pub mod web;

pub use config::Config;
pub use display::{
    DefaultDisplayFormatter, DisplayFormatter, MarkdownDisplayFormatter, PlainDisplayFormatter,
};
pub use editor::open_in_editor;
pub use file_utils::{
    create_template_content, get_time_tracking_dir, get_time_tracking_dir_with_override,
};
pub use time_utils::{format_day_with_date, get_week_dates, parse_weekday};

#[cfg(feature = "webapp")]
pub use web::run_server;
