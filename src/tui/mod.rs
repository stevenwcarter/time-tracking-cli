use crate::{Config, DisplayFormatter};
use anyhow::Result;

pub mod app;
pub mod event;
pub mod popup;
pub mod project_list;
pub mod ui;

pub async fn tui(config: &Config, formatter: Box<dyn DisplayFormatter>) -> Result<()> {
    let terminal = ratatui::init();
    let result = app::App::new(config, formatter).run(terminal).await;
    ratatui::restore();
    result
}
