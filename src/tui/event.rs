#![allow(clippy::new_without_default)]
use anyhow::{Context, Result};
use futures::{Stream, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::{collections::HashMap, io, time::Duration};
use time::Date;
use time_tracking_parser::TimeTrackingData;
use tokio::sync::{mpsc, oneshot};

use crate::data_svc::WeeklySummary;

/// The frequency at which tick events are emitted.
///
/// The event loop no longer redraws on every turn — see `App::run`, which
/// draws only when `App::dirty` is set — so this is not a frame rate. It only
/// has to be often enough for whatever keys off elapsed time rather than off
/// input to feel prompt, and four wakeups a second is imperceptible there.
const TICK_FPS: f64 = 4.0;

/// Representation of all possible events.
#[derive(Clone, Debug)]
pub enum Event {
    /// An event emitted on a regular schedule, `TICK_FPS` times a second.
    ///
    /// Not a frame rate and not a polling hook: the loop redraws only when
    /// something has marked it dirty, and nothing here reaches out to the
    /// disk. `App::tick` uses it for exactly one thing — dropping a
    /// status-line message once its TTL has run out.
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

/// Everything a key binding or a background task can ask the app to do.
///
/// Raising one queues it rather than acting on it, so the state changes all
/// land in one place: `App::apply_sync_event` for the arms that are pure
/// state, `App::handle_app_event` for the few that need to await or to take
/// the terminal apart.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    /// Toggle the help popup
    ToggleHelp,
    /// Dismiss whatever overlay is open
    CloseOverlay,
    /// Open the jump-to-date prompt, empty and ready for input.
    OpenDatePrompt,
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
    /// Toggle between `Mode::Day` and `Mode::Week`.
    ToggleWeekMode,
    /// Select the next project in the weekly rollup
    NextWeekProject,
    /// Select the previous project in the weekly rollup
    PreviousWeekProject,
    /// Select the first project in the weekly rollup
    FirstWeekProject,
    /// Select the last project in the weekly rollup
    LastWeekProject,
    /// Copy the selected project's week — its hours and its day-prefixed
    /// notes — to the clipboard.
    ///
    /// Distinct from [`Self::CopyNotes`], which yanks one *day's* notes
    /// without hours: a timesheet line needs the week's hours, and the two
    /// panes must not share an event or a key pressed in one would move the
    /// other's selection.
    CopyWeekProject,
    /// Put a payload on the system clipboard, reporting `.1` on the status
    /// line once it is there.
    ///
    /// The two fields are `(payload, success_message)`. Separating the intent
    /// from the I/O is what keeps [`ProjectListWidget`] free of a clipboard
    /// handle — the widget knows what should be copied and what to say about
    /// it, and `App` owns the one connection to the system clipboard and the
    /// status line that reports the failure.
    ///
    /// [`ProjectListWidget`]: super::project_list::ProjectListWidget
    CopyToClipboard(String, String),
    /// Copy the active date's summary — with hours, unlike [`Self::CopyNotes`]
    /// — to the clipboard.
    YankDay,
    /// Copy the active week's totals and per-project rollup to the
    /// clipboard.
    YankWeek,
    /// Edit the current date in $EDITOR
    Edit,
    /// Drop to the shell, leaving the process stopped until it is resumed
    /// with `fg`. Unix only; on Windows the handler reports that instead.
    Suspend,
    /// Go to the next date
    NextDate,
    /// Go to the previous date
    PreviousDate,
    /// Go forward a week
    NextWeek,
    /// Go back a week
    PreviousWeek,
    /// Go forward a month, clamping to the target month's last day when it
    /// is shorter than the current one
    NextMonth,
    /// Go back a month, clamping to the target month's last day when it is
    /// shorter than the current one
    PreviousMonth,
    /// Reload the current date from disk, keeping or dropping the calendar's
    /// month memo according to why the reload was asked for.
    ReloadFromDisk(Reload),
    /// Go to today's date
    Today,
    /// Toggle between `Mode::Day` and `Mode::RawFile`.
    ToggleRawFile,
    /// Scroll the raw-file pane down a line.
    ScrollRawFileDown,
    /// Scroll the raw-file pane up a line.
    ScrollRawFileUp,
    /// The background read `App::toggle_raw_file` started for the raw-file
    /// pane finished; carries the generation it was started with — dropped
    /// the same way a superseded `DataLoaded`/`LoadFailed` is when a newer
    /// load has since moved on — and the date it was read for.
    RawFileLoaded(u64, Date, Option<String>),
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

/// Why a reload was asked for, which decides whether the calendar's month
/// memo survives it.
///
/// An enum rather than a naked `bool`, and carried on the event rather than
/// inferred from which dispatch path the event arrived by: the memo used to
/// be dropped in `App::queue_or_apply`, which only *key-produced* events
/// reach, so a reload posted by a background task — Task 21's file watcher —
/// would have refreshed the day and the bar chart while leaving the calendar
/// marker wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reload {
    /// The user moved to another date. The calendar's month memo still
    /// describes the months on screen — a same-month move costing no
    /// directory scan is the whole point of it — so it is kept.
    Navigation,
    /// "Read the disk again": `r`, a finished editor session, or an outside
    /// edit. Nothing on hand can be trusted, the month memo included.
    Rescan,
}

/// Everything one background load produces, applied to the app in one step.
///
/// Loading all four together keeps the day, the calendar and the bar chart
/// from ever showing a mix of two different dates.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadPayload {
    /// The date this load was started for.
    ///
    /// Carried rather than re-read off the app, because `App::active_date`
    /// moves the moment a key is handled while the reload it queues starts a
    /// loop turn or two later — so a payload still carrying the current
    /// generation can land after the date on screen has already changed.
    /// Everything the payload is filed under is keyed off this, not off
    /// whatever is on screen when it lands.
    pub date: Date,
    /// The loaded date's parsed day, or `None` when it has no file yet.
    pub day: Option<TimeTrackingData>,
    /// [`LoadPayload::date`]'s file exactly as it is on disk, for `y`'s
    /// day-summary yank.
    ///
    /// [`crate::display::DisplayFormatter::day_summary`] reparses its own
    /// `content` argument, so only the raw text will do — `day` alone cannot
    /// reconstruct it.
    pub raw: Option<String>,
    /// Dates with tracked hours, for the calendar's markers. Scanned for the
    /// ninety-day window around [`LoadPayload::date`].
    pub populated: Vec<Date>,
    /// Tracked minutes per day of [`LoadPayload::date`]'s week, for the bar
    /// chart.
    ///
    /// A projection of [`LoadPayload::weekly_summary`], not a second
    /// aggregation of it — see [`DataService::get_weekly_data`]'s doc.
    ///
    /// [`DataService::get_weekly_data`]: crate::DataService::get_weekly_data
    pub weekly: HashMap<Date, u32>,
    /// The full weekly rollup `weekly` is a projection of, for `Y`'s
    /// per-project yank.
    pub weekly_summary: WeeklySummary,
}

/// [`WeeklySummary`] does not derive [`PartialEq`] where it is defined in
/// `data_svc.rs`; implemented here, field by field, purely so [`LoadPayload`]
/// — and therefore [`AppEvent`] — can keep deriving it. Every field is
/// already comparable on its own; only the derive on the struct itself is
/// missing.
impl PartialEq for WeeklySummary {
    fn eq(&self, other: &Self) -> bool {
        self.total_minutes == other.total_minutes
            && self.dead_time_minutes == other.dead_time_minutes
            && self.projects == other.projects
            && self.warnings == other.warnings
            && self.per_day == other.per_day
            && self.days == other.days
    }
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

    /// Build a sender directly from a raw channel half.
    ///
    /// `EventHandler::next` takes `&mut self`, so a test that only needs a
    /// sender — a background watcher, say — cannot borrow a whole handler
    /// without also owning its receiver. This lets such a test drive a bare
    /// channel instead.
    #[cfg(test)]
    pub fn from_raw(tx: mpsc::UnboundedSender<Event>) -> Self {
        Self(tx)
    }
}

/// What [`EventHandler`] tells the poller over the pause channel.
#[derive(Debug)]
enum PauseSignal {
    /// Stop reading the terminal, then confirm through the carried sender.
    ///
    /// The confirmation is the point of the variant: "pause" as a bare flag
    /// only ever meant "stop at some later turn", and the caller is about to
    /// hand the terminal to another program *now*.
    Pause(oneshot::Sender<()>),
    /// Start reading the terminal again.
    Resume,
}

/// The poller's confirmation that a pause has taken effect.
///
/// Returned by [`EventHandler::pause`]. Until [`PauseAck::stopped`] resolves
/// the poller may still own a terminal reader, so nothing else may be given
/// the terminal before then.
#[derive(Debug)]
pub struct PauseAck(oneshot::Receiver<()>);

impl PauseAck {
    /// Wait until the poller holds no terminal reader.
    ///
    /// A poller that is gone — its task ended, taking the unanswered request
    /// with it — is not reading the terminal either, so a dropped sender
    /// counts as stopped rather than as an error: the caller is on its way to
    /// run an editor, and there is nothing it could do with the failure.
    ///
    /// A poller that was never started cannot answer at all, and this then
    /// waits forever. `App::run` always calls [`EventHandler::start`] before
    /// its loop; a unit test, which never does, must not await this.
    pub async fn stopped(self) {
        let _ = self.0.await;
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
    pause_sender: mpsc::UnboundedSender<PauseSignal>,
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

    /// Pause event polling (for editor sessions).
    ///
    /// Only *asks*: the poller is another task and has not stopped by the
    /// time this returns. Await the returned [`PauseAck`] before handing the
    /// terminal to anything else — see [`EventTask::run_with`] for what the
    /// acknowledgement guarantees.
    pub fn pause(&mut self) -> PauseAck {
        let (ack, stopped) = oneshot::channel();
        let _ = self.pause_sender.send(PauseSignal::Pause(ack));
        PauseAck(stopped)
    }

    /// Resume event polling
    pub fn resume(&mut self) {
        let _ = self.pause_sender.send(PauseSignal::Resume);
    }

    /// Every pause/resume signal sent so far, oldest first, consuming them.
    ///
    /// A test never calls [`EventHandler::start`], so the [`EventTask`] — and
    /// with it the receiving half of the pause channel — is still sitting in
    /// `self.task`. Reading it is what turns "the editor handover left the
    /// poller paused" into a falsifiable assertion instead of something
    /// inferred from the shape of the code. A paused poller emits neither
    /// ticks nor keys, so the symptom in production is a hang, which no test
    /// can wait for.
    ///
    /// `true` is a pause and `false` a resume.
    #[cfg(test)]
    pub fn drain_pause_signals(&mut self) -> Vec<bool> {
        let Some(task) = self.task.as_mut() else {
            return Vec::new();
        };
        let mut signals = Vec::new();
        while let Ok(signal) = task.pause_receiver.try_recv() {
            signals.push(matches!(signal, PauseSignal::Pause(_)));
        }
        signals
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
#[derive(Debug)]
struct EventTask {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Pause receiver channel.
    pause_receiver: mpsc::UnboundedReceiver<PauseSignal>,
}

impl EventTask {
    /// Constructs a new instance of [`EventThread`].
    fn new(
        sender: mpsc::UnboundedSender<Event>,
        pause_receiver: mpsc::UnboundedReceiver<PauseSignal>,
    ) -> Self {
        Self {
            sender,
            pause_receiver,
        }
    }

    /// Runs the event thread against the real terminal.
    async fn run(self) -> Result<()> {
        self.run_with(crossterm::event::EventStream::new).await
    }

    /// Emits tick events at a fixed rate and forwards terminal events in
    /// between — but only while running, and only through a reader that
    /// exists while running.
    ///
    /// **No reader exists between acknowledging a pause and the resume.**
    /// Not polling the reader is not enough: crossterm's `EventStream` parks
    /// a background thread in a blocking read of the tty whenever it has
    /// returned `Pending`, and that thread stops only when the stream is
    /// dropped. A paused poller that kept its stream therefore went on
    /// racing the editor for input, and whatever it won — the terminal's
    /// reply to the editor's start-up colour query, say — was replayed into
    /// the TUI as keystrokes on resume. So a pause drops the reader *before*
    /// it is acknowledged, and a resume builds a new one.
    ///
    /// The pause channel is a `select!` branch rather than something drained
    /// at the top of the loop, so a pause is acted on when it is sent and not
    /// a tick period later; the caller is waiting on the acknowledgement to
    /// hand the terminal over.
    ///
    /// Generic over how a reader is made only so a test can count readers
    /// without a tty; [`EventTask::run`] is the one production caller.
    async fn run_with<S, F>(mut self, mut make_reader: F) -> Result<()>
    where
        S: Stream<Item = io::Result<CrosstermEvent>> + Unpin,
        F: FnMut() -> S,
    {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut tick = tokio::time::interval(tick_rate);
        // `None` is what "paused" means.
        let mut reader = Some(make_reader());

        loop {
            let Some(active) = reader.as_mut() else {
                match self.pause_receiver.recv().await {
                    // Already stopped, but the sender is still waiting on it.
                    Some(PauseSignal::Pause(ack)) => {
                        let _ = ack.send(());
                    }
                    Some(PauseSignal::Resume) => reader = Some(make_reader()),
                    None => break,
                }
                continue;
            };

            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              signal = self.pause_receiver.recv() => match signal {
                Some(PauseSignal::Pause(ack)) => {
                    // Order is the invariant: the reader is gone before
                    // anyone is told it is.
                    reader = None;
                    let _ = ack.send(());
                }
                Some(PauseSignal::Resume) => {}
                None => break,
              },
              _ = tick.tick() => {
                self.send(Event::Tick);
              }
              Some(Ok(evt)) = active.next() => {
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
    use std::{
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll},
    };
    use tokio::task::JoinHandle;

    /// Stands in for `crossterm::event::EventStream` away from a tty: never
    /// yields an event, and counts itself in and out of a shared tally so a
    /// test can ask how many readers exist at a given instant.
    ///
    /// Existence is the thing worth counting. The real stream's background
    /// thread reads the terminal for as long as the stream is alive, polled
    /// or not, and stops only when it is dropped.
    struct FakeReader(Arc<AtomicUsize>);

    impl FakeReader {
        fn new(live: &Arc<AtomicUsize>) -> Self {
            live.fetch_add(1, Ordering::SeqCst);
            Self(Arc::clone(live))
        }
    }

    impl Drop for FakeReader {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl Stream for FakeReader {
        type Item = io::Result<CrosstermEvent>;

        fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Pending
        }
    }

    /// A poller spawned over [`FakeReader`]s and already parked mid-turn.
    struct RunningPoller {
        handler: EventHandler,
        /// How many [`FakeReader`]s exist right now.
        live: Arc<AtomicUsize>,
        poller: JoinHandle<Result<()>>,
    }

    impl RunningPoller {
        /// Spawn the poller and wait for its first tick.
        ///
        /// The first tick fires immediately, so once it has arrived the task
        /// is parked waiting on the *next* one, a full tick period away. That
        /// is the state a pause has to cut through; a pause sent before the
        /// task had run at all would be seen on its way in and prove nothing.
        async fn spawn() -> Self {
            let mut handler = EventHandler::new();
            let task = handler.task.take().expect("a new handler holds its task");
            let live = Arc::new(AtomicUsize::new(0));
            let counter = Arc::clone(&live);
            let poller = tokio::spawn(task.run_with(move || FakeReader::new(&counter)));

            let first = handler.next().await.expect("the poller is running");
            assert!(matches!(first, Event::Tick));
            assert_eq!(live.load(Ordering::SeqCst), 1, "a running poller reads");

            Self {
                handler,
                live,
                poller,
            }
        }

        fn live_readers(&self) -> usize {
            self.live.load(Ordering::SeqCst)
        }

        /// Pause and wait for the acknowledgement, bounded so a poller that
        /// never answers fails the test instead of hanging the suite.
        async fn pause_acknowledged(&mut self) {
            let ack = self.handler.pause();
            tokio::time::timeout(Duration::from_secs(5), ack.stopped())
                .await
                .expect("the poller must acknowledge a pause");
        }
    }

    /// Regression: `e` handed the terminal to `$EDITOR` while the poller
    /// still owned a crossterm `EventStream`, whose background thread kept
    /// reading the tty and raced the editor for input. nvim asks the terminal
    /// for its background colour on start-up; crossterm sometimes won the
    /// reply (`ESC ] 11;rgb:2828/2c2c/3434 ESC \`) and replayed it as keys
    /// once the editor quit — the `g` of `rgb` opened the jump-to-date prompt
    /// and the rest was typed into it.
    ///
    /// So the acknowledgement has to mean "no reader exists", not "the pause
    /// was noted": the count is taken at the instant the ack resolves, which
    /// is the instant `with_suspended_terminal` gives the terminal away.
    #[tokio::test(start_paused = true)]
    async fn an_acknowledged_pause_leaves_no_reader_to_steal_the_editors_terminal_replies() {
        let mut poller = RunningPoller::spawn().await;

        poller.pause_acknowledged().await;

        assert_eq!(
            poller.live_readers(),
            0,
            "a reader that outlives the ack is still reading the tty under the editor"
        );
    }

    /// The other half of the pause: a poller that dropped its reader and
    /// never built another would leave the TUI deaf after the editor quit.
    #[tokio::test(start_paused = true)]
    async fn resuming_builds_a_fresh_reader() {
        let mut poller = RunningPoller::spawn().await;
        poller.pause_acknowledged().await;

        poller.handler.resume();

        // Sleeps rather than yields between looks: the clock is paused and
        // only advances while every task is idle, so a loop that merely
        // yielded would keep it — and this timeout — from ever moving.
        tokio::time::timeout(Duration::from_secs(5), async {
            while poller.live_readers() != 1 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("a resumed poller must read the terminal again");
    }

    /// A pause used to be looked at only on the top of the loop, so it waited
    /// out the rest of the tick period — up to 250ms in which the editor was
    /// already starting. The fake reader never yields and the clock is
    /// paused, so nothing but the pause itself can wake the poller inside the
    /// bound here; a poller waiting for its next tick misses it.
    #[tokio::test(start_paused = true)]
    async fn a_pause_is_acknowledged_without_waiting_for_a_tick_or_a_key() {
        let mut poller = RunningPoller::spawn().await;
        let tick_period = Duration::from_secs_f64(1.0 / TICK_FPS);

        let ack = poller.handler.pause();

        tokio::time::timeout(tick_period / 2, ack.stopped())
            .await
            .expect("a pause must not wait for the next tick");
    }

    /// Every pause gets its answer, including one sent to a poller that is
    /// already paused — an unanswered ack would hang its caller for good.
    #[tokio::test(start_paused = true)]
    async fn a_second_pause_while_paused_is_still_acknowledged() {
        let mut poller = RunningPoller::spawn().await;
        poller.pause_acknowledged().await;

        poller.pause_acknowledged().await;

        assert_eq!(poller.live_readers(), 0);
    }

    /// A paused poller used to sleep-and-recheck forever, never noticing the
    /// handler had gone. It now waits on the pause channel itself, so the
    /// handler dropping its end is what ends the task.
    #[tokio::test(start_paused = true)]
    async fn a_paused_poller_exits_when_its_handler_is_dropped() {
        let mut poller = RunningPoller::spawn().await;
        poller.pause_acknowledged().await;

        drop(poller.handler);

        tokio::time::timeout(Duration::from_secs(5), poller.poller)
            .await
            .expect("a paused poller must notice its handler going away")
            .expect("the poller task must not panic")
            .expect("the poller exits cleanly");
    }

    /// The property a watcher task depends on: a sender that outlives the
    /// borrow of the handler and can be handed to as many tasks as needed.
    #[test]
    fn a_cloned_sender_queues_events_without_borrowing_the_handler() {
        let mut handler = EventHandler::new();
        let tx = handler.sender();
        let tx2 = tx.clone();

        tx.send(AppEvent::ReloadFromDisk(Reload::Rescan));
        tx2.send(AppEvent::Today);

        assert!(matches!(
            handler.try_next(),
            Some(Event::App(AppEvent::ReloadFromDisk(Reload::Rescan)))
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
