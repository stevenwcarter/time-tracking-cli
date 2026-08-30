//! The one place that knows which terminal modes the TUI has turned on.
//!
//! Before this module the mode set lived in four places — `tui()`'s
//! `ratatui::init`/`restore` pair and the `$EDITOR` handover's own
//! leave/enter pair — and the two were only equal by inspection. Mouse
//! capture is the mode that made that untenable: a mode the editor handover
//! forgot to drop is one the editor session inherits.

use std::future::Future;
use std::io::stdout;

use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use super::event::EventHandler;

/// Every terminal mode the TUI turns on beyond the terminal's defaults.
///
/// Held by value and passed to [`with_suspended_terminal`] rather than read
/// from a global, so the suspend path cannot drift from what start-up
/// actually enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalModes {
    /// Is mouse capture on? Governed by the `mouse` config key.
    pub mouse: bool,
}

impl TerminalModes {
    /// Take the terminal: alternate screen, raw mode, and mouse capture when
    /// it is enabled.
    pub fn enter(self) -> Result<()> {
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        if self.mouse {
            stdout().execute(EnableMouseCapture)?;
        }
        Ok(())
    }

    /// Give the terminal back, innermost mode first — the exact inverse of
    /// [`TerminalModes::enter`].
    pub fn leave(self) -> Result<()> {
        if self.mouse {
            stdout().execute(DisableMouseCapture)?;
        }
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        Ok(())
    }
}

/// Holds the event poller paused for as long as it lives, and resumes it on
/// the way out — including the way out an `?` takes.
///
/// A `Drop` guard rather than a matching `resume()` call at the bottom of
/// [`with_suspended_terminal`] because the failure mode is invisible: a
/// paused poller emits nothing at all, so a missed resume does not error, it
/// hangs. The pairing has to be structural, not remembered.
struct PausedPoller<'a>(&'a mut EventHandler);

impl<'a> PausedPoller<'a> {
    /// Pause `events` until the returned guard is dropped.
    fn new(events: &'a mut EventHandler) -> Self {
        events.pause();
        Self(events)
    }
}

impl Drop for PausedPoller<'_> {
    fn drop(&mut self) {
        self.0.resume();
    }
}

/// Hand the terminal back to the shell in cooked mode, run `body`, then take
/// the terminal back and redraw from scratch.
///
/// The poller is paused throughout: crossterm and whatever `body` hands the
/// terminal to must not both be reading stdin.
///
/// **`body`'s error does not skip the restore.** The result is combined the
/// way the editor handover always did it — the body's error is what the
/// caller sees, but the terminal is put back either way. Returning early on
/// a body error would leave a cooked terminal under a running TUI.
pub async fn with_suspended_terminal<B, T, F, Fut>(
    events: &mut EventHandler,
    terminal: &mut Terminal<B>,
    modes: TerminalModes,
    body: F,
) -> Result<T>
where
    B: Backend,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let _paused = PausedPoller::new(events);

    let left = modes.leave();
    let out = body().await;
    let restored = left.and_then(|()| {
        modes.enter()?;
        // Whatever ran while we were away wrote over the alternate screen.
        terminal.clear()?;
        Ok(())
    });

    match restored {
        Ok(()) => out,
        Err(e) => out.and(Err(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::event::EventHandler;
    use ratatui::{Terminal, backend::TestBackend};

    /// The helper must pause the poller on the way in and resume it on the
    /// way out. A missed resume does not error — it hangs — so this is the
    /// only way the pairing is checked. Mirrors the existing editor
    /// regression test in `app.rs`.
    ///
    /// `with_suspended_terminal`'s own result is discarded: `modes.leave()`
    /// and `modes.enter()` call real crossterm mode changes, and
    /// `enable_raw_mode`/`disable_raw_mode` return `Err` when stdout is not
    /// a tty, which is always true under `cargo test`. Only the pause
    /// signals — which do not touch the real terminal — are asserted on.
    #[tokio::test]
    async fn suspending_pauses_and_resumes_the_poller() {
        let mut events = EventHandler::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).expect("test backend");
        let modes = TerminalModes { mouse: false };

        let _ =
            with_suspended_terminal(&mut events, &mut terminal, modes, || async { Ok(()) }).await;

        assert_eq!(events.drain_pause_signals(), vec![true, false]);
    }

    /// A body that fails must still leave the terminal restored and the
    /// poller resumed: the error is reported, the terminal is put back.
    #[tokio::test]
    async fn a_failing_body_still_resumes_the_poller() {
        let mut events = EventHandler::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).expect("test backend");
        let modes = TerminalModes { mouse: false };

        let result: anyhow::Result<()> =
            with_suspended_terminal(&mut events, &mut terminal, modes, || async {
                Err(anyhow::anyhow!("body failed"))
            })
            .await;

        assert!(result.is_err(), "the body's error must be reported");
        assert_eq!(events.drain_pause_signals(), vec![true, false]);
    }

    #[test]
    fn modes_are_copy_and_compare_by_value() {
        let a = TerminalModes { mouse: true };
        let b = a;
        assert_eq!(a, b);
    }
}
