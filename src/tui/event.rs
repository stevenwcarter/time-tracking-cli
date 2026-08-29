#![allow(clippy::new_without_default)]
use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::{collections::HashMap, time::Duration};
use time::Date;
use time_tracking_parser::TimeTrackingData;
use tokio::sync::mpsc;

/// The frequency at which tick events are emitted.
const TICK_FPS: f64 = 30.0;

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// An event that is emitted on a regular schedule.
    ///
    /// Use this event to run any code which has to run outside of being a direct response to a user
    /// event. e.g. polling exernal systems, updating animations, or rendering the UI based on a
    /// fixed frame rate.
    Tick,
    /// Crossterm events.
    ///
    /// These events are emitted by the terminal.
    Crossterm(CrosstermEvent),
    /// Application events.
    ///
    /// Use this event to emit custom events that are specific to your application.
    App(AppEvent),
}

/// Application events.
///
/// You can extend this enum with your own custom events.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    /// Toggle the help popup
    ToggleHelp,
    /// Dismiss whatever overlay is open
    CloseOverlay,
    /// Toggle whether the bar chart is zoomed
    ToggleZoomBar,
    /// Select the next project in the day view's list
    NextProject,
    /// Select the previous project in the day view's list
    PreviousProject,
    /// Select the first project in the day view's list
    FirstProject,
    /// Select the last project in the day view's list
    LastProject,
    /// Copy the selected project's notes to the clipboard
    CopyNotes,
    /// Edit the current date in $EDITOR
    Edit,
    /// Go to the next date
    NextDate,
    /// Go to the previous date
    PreviousDate,
    /// Reload the current date from disk
    ReloadFromDisk,
    /// Go to today's date
    Today,
    /// A background load finished; carries the generation it was started with.
    ///
    /// The payload is boxed because it dwarfs every other variant, and every
    /// `AppEvent` — including the ones a held-down key floods the queue with —
    /// would otherwise be as large as a whole day's parsed data.
    DataLoaded(u64, Box<LoadPayload>),
    /// A background load failed; carries the generation it was started with.
    LoadFailed(u64, String),
    /// Quit the application.
    Quit,
}

/// Everything one background load produces, applied to the app in one step.
///
/// Loading all three together keeps the day, the calendar and the bar chart
/// from ever showing a mix of two different dates.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadPayload {
    /// The active date's parsed day, or `None` when it has no file yet.
    pub day: Option<TimeTrackingData>,
    /// Dates with tracked hours, for the calendar's markers.
    pub populated: Vec<Date>,
    /// Tracked minutes per day of the active week, for the bar chart.
    pub weekly: HashMap<Date, u32>,
}

/// A cloneable handle for queueing [`AppEvent`]s.
///
/// [`EventHandler::send`] borrows the handler mutably, so only whoever owns
/// the event loop can use it. This borrows itself immutably and clones, which
/// is what lets a spawned task report back into the loop.
#[derive(Clone, Debug)]
pub struct AppEventSender(mpsc::UnboundedSender<Event>);

impl AppEventSender {
    /// Queue `app_event` for the next turn of the application's event loop.
    pub fn send(&self, app_event: AppEvent) {
        // Shutting down the app drops the receiver, so a failed send here is
        // an in-flight task finding the loop already gone. Expected; ignore.
        let _ = self.0.send(Event::App(app_event));
    }
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Pause sender to control the event task
    pause_sender: mpsc::UnboundedSender<bool>,
    /// The poller, held until [`EventHandler::start`] spawns it.
    task: Option<EventTask>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`].
    ///
    /// This only wires up the channels. No task is spawned and no terminal is
    /// opened until [`EventHandler::start`] is called, so an `EventHandler` —
    /// and the [`App`](super::app::App) that owns one — can be constructed
    /// outside a Tokio runtime and away from a tty.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let (pause_sender, pause_receiver) = mpsc::unbounded_channel();
        Self {
            task: Some(EventTask::new(sender.clone(), pause_receiver)),
            sender,
            receiver,
            pause_sender,
        }
    }

    /// Spawn the task that reads crossterm events and emits ticks.
    ///
    /// Called once, by `App::run`, immediately before the event loop starts. A
    /// second call does nothing, so the poller can never be spawned twice.
    pub fn start(&mut self) {
        if let Some(task) = self.task.take() {
            tokio::spawn(async { task.run().await });
        }
    }

    /// A handle a background task can report back through.
    ///
    /// Unlike [`EventHandler::send`] this needs neither a mutable borrow nor
    /// the handler itself, so it can be moved into a spawned task.
    pub fn sender(&self) -> AppEventSender {
        AppEventSender(self.sender.clone())
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub async fn next(&mut self) -> Result<Event> {
        // Nothing else can catch a missing `start()`: `App::run` takes a
        // `DefaultTerminal`, so no test can drive the real loop, and the only
        // symptom in production is a TUI hanging on an empty channel. `start`
        // is take-and-spawn, so an absent task means it ran.
        debug_assert!(
            self.task.is_none(),
            "EventHandler::start() was never called — the poller is not running and next() will \
             block forever"
        );
        self.receiver
            .recv()
            .await
            .context("Failed to receive event")
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }

    /// Pop an already-queued event without awaiting, or `None` when the queue
    /// is empty.
    ///
    /// Lets a test drive the app by hand — send a key, then drain whatever it
    /// queued — without standing up the event loop.
    #[cfg(test)]
    pub fn try_next(&mut self) -> Option<Event> {
        self.receiver.try_recv().ok()
    }

    /// Pause event polling (for editor sessions)
    pub fn pause(&mut self) {
        let _ = self.pause_sender.send(true);
    }

    /// Resume event polling
    pub fn resume(&mut self) {
        let _ = self.pause_sender.send(false);
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
#[derive(Debug)]
struct EventTask {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Pause receiver channel.
    pause_receiver: mpsc::UnboundedReceiver<bool>,
}

impl EventTask {
    /// Constructs a new instance of [`EventThread`].
    fn new(
        sender: mpsc::UnboundedSender<Event>,
        pause_receiver: mpsc::UnboundedReceiver<bool>,
    ) -> Self {
        Self {
            sender,
            pause_receiver,
        }
    }

    /// Runs the event thread.
    ///
    /// This function emits tick events at a fixed rate and polls for crossterm events in between.
    async fn run(mut self) -> Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        let mut paused = false;

        loop {
            // Check for pause/resume signals
            while let Ok(pause_signal) = self.pause_receiver.try_recv() {
                paused = pause_signal;
            }

            if paused {
                // When paused, just sleep and check for resume signals
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                self.send(Event::Tick);
              }
              Some(Ok(evt)) = crossterm_event => {
                self.send(Event::Crossterm(evt));
              }
            };
        }
        Ok(())
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property a watcher task depends on: a sender that outlives the
    /// borrow of the handler and can be handed to as many tasks as needed.
    #[test]
    fn a_cloned_sender_queues_events_without_borrowing_the_handler() {
        let mut handler = EventHandler::new();
        let tx = handler.sender();
        let tx2 = tx.clone();

        tx.send(AppEvent::ReloadFromDisk);
        tx2.send(AppEvent::Today);

        assert!(matches!(
            handler.try_next(),
            Some(Event::App(AppEvent::ReloadFromDisk))
        ));
        assert!(matches!(
            handler.try_next(),
            Some(Event::App(AppEvent::Today))
        ));
        assert!(handler.try_next().is_none());
    }

    /// A dropped sender must not close the queue while the handler lives, or
    /// the first task to finish would take the event loop down with it.
    #[test]
    fn dropping_a_sender_leaves_the_queue_open() {
        let mut handler = EventHandler::new();
        let tx = handler.sender();

        tx.send(AppEvent::Today);
        drop(tx);

        assert!(matches!(
            handler.try_next(),
            Some(Event::App(AppEvent::Today))
        ));
        handler.sender().send(AppEvent::Quit);
        assert!(matches!(
            handler.try_next(),
            Some(Event::App(AppEvent::Quit))
        ));
    }

    /// Pins the `start()` call in `App::run`: dropping it would otherwise hang
    /// the real TUI on an empty channel with the whole suite still green.
    ///
    /// Debug only — `debug_assert!` compiles out of a release test build, and
    /// `next()` would then block forever instead of panicking.
    #[cfg(debug_assertions)]
    #[tokio::test]
    #[should_panic(expected = "EventHandler::start() was never called")]
    async fn awaiting_an_event_before_start_panics() {
        let mut handler = EventHandler::new();
        // Bounded, so deleting the assertion fails this test rather than
        // hanging the suite on the very empty channel it guards against.
        let _ = tokio::time::timeout(Duration::from_secs(5), handler.next()).await;
    }
}
