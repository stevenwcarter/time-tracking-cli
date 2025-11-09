use crate::{Config, DisplayFormatter};
use anyhow::Result;

pub mod app;
pub mod event;
pub mod project_list;
pub mod ui;
pub mod widgets;

pub async fn tui(config: &Config, formatter: Box<dyn DisplayFormatter>) -> Result<()> {
    let terminal = ratatui::init();
    let result = app::App::new(config, formatter).run(terminal).await;
    ratatui::restore();
    result
}
