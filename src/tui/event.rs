#![allow(clippy::new_without_default)]
use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::time::Duration;
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
    /// Toggle whether the bar chart is zoomed
    ToggleZoomBar,
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
    /// Quit the application.
    Quit,
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
