use anyhow::Result;

pub mod app;
pub mod event;
pub mod project_list;
pub mod ui;
pub mod widgets;

pub async fn tui() -> Result<()> {
    let terminal = ratatui::init();
    let result = app::App::new().run(terminal).await;
    ratatui::restore();
    result
}
