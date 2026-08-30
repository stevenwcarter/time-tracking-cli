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
    ///
    /// That inverse order is a deliberate choice, not a leftover. The
    /// `$EDITOR` handover this module replaced unwound in the opposite
    /// order — `LeaveAlternateScreen` before `disable_raw_mode()` — while
    /// `ratatui::restore()` (what actually runs at shutdown) disables raw
    /// mode first and leaves the alternate screen second, the order used
    /// here. So the codebase already had two different answers for how to
    /// undo one conceptual operation; this regularises on the one
    /// `ratatui::restore()` uses. The change is not observable — nothing
    /// renders between the two calls either way — but it is a real change
    /// from the code this module replaced, recorded here rather than left
    /// to be rediscovered.
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

/// Combine the body's result with the terminal-restore result the way the
/// editor handover always did it: the body's error is what the caller sees
/// when both fail, but a restore failure must still surface when the body
/// itself succeeded.
///
/// Pulled out of [`with_suspended_terminal`] and kept pure — no terminal, no
/// `EventHandler` — so all four (body, restore) combinations can be checked
/// directly. That is the whole of the "a failing body still restores the
/// terminal" semantics; [`with_suspended_terminal`] itself only wires it to
/// the real leave/enter calls.
fn combine_results<T>(body: Result<T>, restore: Result<()>) -> Result<T> {
    match restore {
        Ok(()) => body,
        Err(e) => body.and(Err(e)),
    }
}

/// Hand the terminal back to the shell in cooked mode, run `body`, then take
/// the terminal back and redraw from scratch.
///
/// The poller is paused throughout: crossterm and whatever `body` hands the
/// terminal to must not both be reading stdin.
///
/// **`body`'s error does not skip the restore.** The result is combined by
/// [`combine_results`] the way the editor handover always did it — the
/// body's error is what the caller sees, but the terminal is put back
/// either way. Returning early on a body error would leave a cooked
/// terminal under a running TUI.
///
/// A failure of [`TerminalModes::leave`] *does* skip the re-enter, and
/// deliberately: `leave` is the pair of `enter`, so re-entering modes that
/// were never fully left is a worse guess than leaving them as they are,
/// and the error is reported rather than swallowed. The cost is real but
/// bounded — a partial leave (say `DisableMouseCapture` writing but
/// `disable_raw_mode` failing) leaves mouse capture off for the rest of the
/// session with nothing to turn it back on. Nothing is stranded: the
/// process still exits through `ratatui::restore`.
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

    combine_results(out, restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the editor handover paused the poller and left the
    /// alternate screen before doing anything fallible, and the `?` on
    /// `create_day_file_if_not_exists` skipped both the restore and
    /// `events.resume()`. A paused `EventTask` emits neither ticks nor keys
    /// and never notices its receiver closing, so the loop's next
    /// `events.next().await` blocked forever — a wedged app outside the
    /// alternate screen with raw mode off, and under `--tui --serve` a
    /// process that never exited.
    ///
    /// Driven through `PausedPoller` rather than through
    /// `with_suspended_terminal` itself on purpose: `with_suspended_terminal`
    /// calls `enable_raw_mode` (by way of [`TerminalModes::enter`]), and
    /// crossterm reaches for `/dev/tty` directly rather than for the
    /// captured stdout — falling back to the captured stdout only when
    /// `/dev/tty` is absent, which is why this passes harmlessly in a
    /// sandbox with no controlling tty but would leave a developer's real
    /// terminal in raw mode if it ran there. The guard is the seam that
    /// carries the invariant, and it is the seam this asserts on — never
    /// drive this through `with_suspended_terminal` in a test.
    #[test]
    fn the_pause_guard_resumes_the_poller_on_the_way_out_of_a_failure() {
        let mut events = EventHandler::new();

        let handover: Result<()> = (|| {
            let _paused = PausedPoller::new(&mut events);
            anyhow::bail!("create_day_file_if_not_exists: Read-only file system")
        })();

        assert!(handover.is_err(), "the fixture must model a failure");
        assert_eq!(
            events.drain_pause_signals(),
            vec![true, false],
            "a failed handover must leave the poller running, not paused"
        );
    }

    #[test]
    fn modes_are_copy_and_compare_by_value() {
        let a = TerminalModes { mouse: true };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn combine_results_passes_through_a_double_success() {
        let combined = combine_results(Ok(42), Ok(()));
        assert_eq!(combined.expect("both succeeded"), 42);
    }

    #[test]
    fn combine_results_surfaces_the_bodys_error_when_restore_succeeds() {
        let combined: Result<()> = combine_results(Err(anyhow::anyhow!("body failed")), Ok(()));
        assert_eq!(combined.unwrap_err().to_string(), "body failed");
    }

    #[test]
    fn combine_results_surfaces_the_restore_error_when_the_body_succeeds() {
        let combined = combine_results(Ok(42), Err(anyhow::anyhow!("restore failed")));
        assert_eq!(combined.unwrap_err().to_string(), "restore failed");
    }

    #[test]
    fn combine_results_prefers_the_bodys_error_when_both_fail() {
        let combined: Result<()> = combine_results(
            Err(anyhow::anyhow!("body failed")),
            Err(anyhow::anyhow!("restore failed")),
        );
        assert_eq!(
            combined.unwrap_err().to_string(),
            "body failed",
            "the body's error is what the caller sees, per with_suspended_terminal's contract"
        );
    }
}
