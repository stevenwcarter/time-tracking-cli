//! What is on screen, and who gets first refusal on a keypress.
//!
//! The TUI draws exactly one [`Mode`] plus at most one [`Overlay`] on top of
//! it. Keys are offered to the overlay first, then to the mode, then to the
//! global bindings; each layer answers with a [`Handled`] saying whether the
//! key stops there.

use super::event::AppEvent;

/// The view currently filling the screen.
///
/// Exactly one mode is active at a time; [`Overlay`] is what gets drawn over
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A single day: calendar, weekly bar chart and the project list.
    Day,
    /// The whole week's projects rolled up. Task 20 builds the view.
    Week,
    /// The weekly bar chart, full screen.
    ZoomedWeek,
    /// The active date's file as it sits on disk. Task 16 builds the view.
    RawFile,
}

/// A modal layer drawn over the active [`Mode`].
///
/// Overlays are modal in the strict sense: while one is open it is the only
/// layer that sees a key, and anything it does not handle is swallowed rather
/// than reaching the mode or the global bindings behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    /// The help popup.
    Help,
    /// A one-line prompt for a date to jump to, holding what has been typed
    /// so far. Task 17 makes it reachable.
    DatePrompt(String),
}

/// A key layer's verdict.
///
/// `Ignored` falls through to the next layer; `Consumed` and `Emit` stop
/// there. `Emit` exists so a layer can change application state without
/// reaching for the event sender itself, which is what keeps the layers
/// testable in isolation.
#[derive(Clone, Debug, PartialEq)]
pub enum Handled {
    /// The layer acted on the key; nothing further happens.
    Consumed,
    /// The layer turned the key into an application event to be queued.
    Emit(AppEvent),
    /// The layer had no use for the key.
    Ignored,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::{app::App, context::TuiContext, testing::fixture_day};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use time::macros::date;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn plain(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// The row the project list has selected, if there is a list at all.
    fn selection(app: &App) -> Option<usize> {
        app.project_list_widget.as_ref()?.selected_item()
    }

    /// A day view with three projects, opened on a fixed date.
    fn day_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(date!(2026 - 08 - 24))
            .with_data(fixture_day())
    }

    #[test]
    fn overlay_swallows_keys_the_mode_would_otherwise_handle() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(key('l')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        // Neither the date behind the popup nor the hidden list may move.
        assert_eq!(app.active_date, date!(2026 - 08 - 24));
        assert_eq!(
            selection(&app),
            Some(0),
            "j must not move the list hidden behind the popup"
        );
    }

    /// Guards the test above from passing vacuously: the same two keys must
    /// still do their jobs once the overlay is gone.
    #[test]
    fn the_same_keys_work_with_no_overlay_open() {
        let mut app = day_app();

        app.handle_key_events(key('l')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.active_date, date!(2026 - 08 - 25));
        assert_eq!(selection(&app), Some(1));
    }

    #[test]
    fn esc_closes_the_overlay_instead_of_quitting() {
        let mut app = App::new(TuiContext::for_test());
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(plain(KeyCode::Esc)).unwrap();
        app.drain_pending_events();

        assert!(app.overlay.is_none(), "Esc should close the overlay");
        assert!(app.running, "Esc must not quit while an overlay is open");
    }

    #[test]
    fn q_closes_the_overlay_instead_of_quitting() {
        let mut app = App::new(TuiContext::for_test());
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(key('q')).unwrap();
        app.drain_pending_events();

        assert!(app.overlay.is_none());
        assert!(app.running);
    }

    #[test]
    fn esc_quits_when_no_overlay_is_open() {
        let mut app = App::new(TuiContext::for_test());
        app.handle_key_events(plain(KeyCode::Esc)).unwrap();
        app.drain_pending_events();
        assert!(!app.running);
    }

    /// Raw mode delivers Ctrl-C as a key, not a signal, so an overlay that
    /// swallowed it would leave the user unable to quit without dismissing
    /// the popup first.
    #[test]
    fn ctrl_c_quits_even_with_an_overlay_open() {
        let mut app = App::new(TuiContext::for_test());
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
            .unwrap();
        app.drain_pending_events();

        assert!(!app.running);
    }

    #[test]
    fn question_mark_toggles_the_help_overlay() {
        let mut app = App::new(TuiContext::for_test());

        app.handle_key_events(key('?')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.overlay, Some(Overlay::Help));

        app.handle_key_events(key('?')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.overlay, None);
    }

    #[test]
    fn f_toggles_the_zoomed_week_mode() {
        let mut app = day_app();
        assert_eq!(app.mode, Mode::Day);

        app.handle_key_events(key('f')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.mode, Mode::ZoomedWeek);

        app.handle_key_events(key('f')).unwrap();
        app.drain_pending_events();
        assert_eq!(app.mode, Mode::Day);
    }

    /// The project list belongs to the day view, so its keys must not reach
    /// it from another mode — `j` there is free for Task 16 to bind.
    #[test]
    fn the_project_list_only_sees_keys_in_day_mode() {
        let mut app = day_app();
        app.mode = Mode::ZoomedWeek;

        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(selection(&app), Some(0));
    }
}
