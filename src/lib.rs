pub mod config;
pub mod data_svc;
pub mod display;
pub mod editor;
pub mod file_utils;
pub mod logging;
pub mod time_utils;

#[cfg(feature = "webapp")]
pub mod context;
#[cfg(feature = "webapp")]
pub mod graphql;
#[cfg(feature = "tui")]
pub mod tui;
#[cfg(feature = "webapp")]
pub mod web;

pub static DATE_FORMAT: &[time::format_description::BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]");

pub use config::Config;
pub use data_svc::DataService;
pub use display::{
    DefaultDisplayFormatter, DisplayFormatter, MarkdownDisplayFormatter, PlainDisplayFormatter,
    show_single_day, show_weekly_summary,
};
pub use editor::open_in_editor;
pub use file_utils::{create_template_content, get_time_tracking_dir};
use time::macros::format_description;
pub use time_utils::{format_day_with_date, get_week_dates, parse_weekday};

#[cfg(feature = "webapp")]
pub use web::run_server;
