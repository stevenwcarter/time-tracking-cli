use anyhow::Result;

use crate::Config;

pub mod app;
mod band;
pub mod context;
pub mod event;
pub mod keymap;
pub mod mode;
pub mod project_list;
pub mod theme;
pub mod ui;
pub mod week_list;
pub mod widgets;

#[cfg(test)]
pub mod testing;

pub async fn tui() -> Result<()> {
    let config = Config::get();
    let ctx = context::TuiContext::from_config(config)?;
    let terminal = ratatui::init();
    let result = app::App::new(ctx)
        .with_active_date(config.date)
        .run(terminal)
        .await;
    ratatui::restore();
    result
}
