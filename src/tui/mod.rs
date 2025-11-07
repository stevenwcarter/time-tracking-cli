use time::Date;

use crate::{Config, DisplayFormatter};
use anyhow::Result;

pub mod app;
pub mod event;
pub mod ui;

pub async fn tui(config: &Config, date: Date, formatter: Box<dyn DisplayFormatter>) -> Result<()> {
    let terminal = ratatui::init();
    let result = app::App::new(config, date, formatter).run(terminal).await;
    ratatui::restore();
    result
}
