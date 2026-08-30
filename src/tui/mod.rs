use anyhow::Result;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use std::io::stdout;

use crate::Config;

pub mod app;
mod band;
pub mod context;
pub mod event;
pub mod keymap;
pub mod layout_rects;
pub mod mode;
pub mod project_list;
pub mod terminal;
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
    // `ratatui::init` installed a hook that restores raw mode and leaves the
    // alternate screen — but it knows nothing about mouse capture, so a
    // panic would drop the user into a shell that emits escape sequences on
    // every drag. Ours is installed *after* ratatui's, so it runs *first*,
    // dropping capture while the alternate screen is still up.
    //
    // Enabling capture is best-effort, not `?`: `ratatui::restore()` below
    // must be reachable from every path that got past `ratatui::init()`, so
    // a failed escape-code write is not worth stranding the terminal in raw
    // mode over. Same reasoning as the panic hook's own `let _ =`.
    if ctx.mouse {
        let _ = stdout().execute(EnableMouseCapture);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = stdout().execute(DisableMouseCapture);
            previous(info);
        }));
    }
    let mouse = ctx.mouse;
    let result = app::App::new(ctx)
        .with_active_date(config.date)
        .run(terminal)
        .await;
    // Best-effort for the same reason as the enable above: this must not
    // skip `ratatui::restore()`, and must not replace whatever `result`
    // `App::run` produced with a cleanup error of its own.
    if mouse {
        let _ = stdout().execute(DisableMouseCapture);
    }
    ratatui::restore();
    result
}
