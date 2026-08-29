use crate::time_utils::get_week_dates;
use std::{collections::HashMap, io::stdout};

use crate::{DataService, editor::open_in_editor};

use super::{
    context::TuiContext,
    event::{AppEvent, Event, EventHandler, LoadPayload},
    keymap,
    mode::{Handled, Mode, Overlay},
    project_list::ProjectListWidget,
};
use anyhow::{Context, Result};
use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Terminal,
    backend::Backend,
    crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
};
use time::{Date, OffsetDateTime};
use time_tracking_parser::TimeTrackingData;
use tokio::task::JoinHandle;

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Does the screen need repainting before the next event is awaited?
    ///
    /// [`App::run`] draws only when this is set, so a state change that fails
    /// to set it shows up as a *frozen* screen — no error, no panic, just a UI
    /// that quietly stops updating. It is therefore set centrally, at the
    /// three seams every change passes through — [`App::handle_key_events`],
    /// [`App::handle_app_event`] and [`App::apply_sync_event`] — rather than
    /// at each individual call site. A terminal resize sets it too: nothing in
    /// the app changed, but the frame on screen was laid out for another size.
    ///
    /// Private for the same reason `load_task` is: a flag whose *clearing*
    /// freezes the UI has no business being reachable from outside this
    /// module. The in-module tests set it directly.
    dirty: bool,
    /// The view currently filling the screen.
    pub mode: Mode,
    /// The modal layer drawn over `mode`, if any.
    ///
    /// While this is `Some` the overlay is the only layer that sees a key;
    /// see [`App::handle_key_events`].
    pub overlay: Option<Overlay>,
    /// Is a load of the active date's data currently in flight
    pub loading: bool,
    /// Which load is the current one; see [`App::spawn_load`].
    ///
    /// Bumped every time a load starts, and stamped on the event that load
    /// reports back with, so a result that arrives after the user has already
    /// moved on can be recognised as superseded and dropped.
    pub load_gen: u64,
    /// The load [`App::spawn_load`] last started, held so a newer one can
    /// abort it instead of leaving it to run out unwatched.
    load_task: Option<JoinHandle<()>>,
    /// Current active date
    pub active_date: Date,
    /// The seven dates of `active_date`'s week, per `ctx.week_start_day`.
    ///
    /// Derived once whenever `active_date` changes rather than on every
    /// frame; the weekly bar chart just borrows it.
    pub week_dates: [Date; 7],
    /// Event handler.
    pub events: EventHandler,
    /// Time tracking data for current date
    pub data: Option<TimeTrackingData>,
    /// Project list widget
    pub project_list_widget: Option<ProjectListWidget>,
    /// Populated dates (have hours)
    pub populated_dates: Vec<Date>,
    /// Calendar markers already scanned, keyed by `(year, month)` of the month
    /// that was *on screen* when the scan ran.
    ///
    /// The calendar covers the month either side of the displayed one, so one
    /// entry holds the whole ninety-day window and a date change inside the
    /// same month can reuse it outright — twenty-nine days out of thirty the
    /// rescan would return the same list. See [`App::month_scan_needed`] for
    /// the lookup and [`App::run_editor`]/[`App::queue_or_apply`] for the two
    /// places it is dropped.
    pub month_memo: HashMap<(i32, u8), Vec<Date>>,
    /// Weekly time tracking data (Date -> minutes)
    pub weekly_data: HashMap<Date, u32>,
    /// Reader for the day files under `ctx.data_dir`
    pub data_svc: DataService,
    /// Environment the app runs against (week start, data dir, theme, ...)
    pub ctx: TuiContext,
}

impl App {
    /// Constructs a new instance of [`App`], opened on today's date.
    ///
    /// Everything the app needs from the environment arrives in `ctx`; `App`
    /// never reads the global `Config` singleton, so it stays constructible
    /// from a test.
    pub fn new(ctx: TuiContext) -> Self {
        let active_date = today();
        let week_dates = get_week_dates(&active_date, ctx.week_start_day);
        let data_svc = DataService::new_with_dir(
            DataService::DEFAULT_CACHE_TIMEOUT_SECONDS,
            ctx.data_dir.clone(),
            ctx.parse_settings(),
        );
        Self {
            running: true,
            // Nothing has been drawn yet, so the loop's first turn must paint.
            dirty: true,
            mode: Mode::Day,
            overlay: None,
            loading: false,
            load_gen: 0,
            load_task: None,
            active_date,
            week_dates,
            events: EventHandler::new(),
            data: None,
            project_list_widget: None,
            populated_dates: Vec::new(),
            month_memo: HashMap::new(),
            weekly_data: HashMap::new(),
            data_svc,
            ctx,
        }
    }

    /// Open on `date` rather than today.
    #[must_use]
    pub fn with_active_date(mut self, date: Date) -> Self {
        self.active_date = date;
        self.week_dates = get_week_dates(&date, self.ctx.week_start_day);
        self
    }

    /// Seed the app with already-parsed data, as a disk load would.
    #[cfg(test)]
    #[must_use]
    pub fn with_data(mut self, data: TimeTrackingData) -> Self {
        self.set_day_data(Some(data));
        self
    }

    /// Seed the app by parsing the body of a day file.
    #[cfg(test)]
    #[must_use]
    pub fn with_raw_content(self, content: &str) -> Self {
        use time_tracking_parser::parse_time_tracking_data;

        self.with_data(parse_time_tracking_data(content, None, None))
    }

    /// Seed the per-day minutes the weekly bar chart draws.
    #[cfg(test)]
    #[must_use]
    pub fn with_weekly_data(mut self, weekly_data: HashMap<Date, u32>) -> Self {
        self.weekly_data = weekly_data;
        self
    }

    /// Seed the dates the calendar marks as having tracked hours.
    #[cfg(test)]
    #[must_use]
    pub fn with_populated_dates(mut self, populated_dates: Vec<Date>) -> Self {
        self.populated_dates = populated_dates;
        self
    }

    /// Run the application's main loop.
    ///
    /// The loop turns once per event but draws only when `dirty` is set, so an
    /// idle terminal costs the tick rate in wakeups a second and no rendering
    /// at all.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Reads no longer create the data directory (see
        // `DataService::get_file_path`), so this is where a fresh install gets
        // one. Not fatal if it fails: every read still answers "no file", and
        // pressing `e` reports the real error when it tries to write.
        if let Err(e) = self.data_svc.ensure_data_dir().await {
            tracing::warn!("could not create the data directory: {e}");
        }
        // Nothing polls the terminal until now; see `EventHandler::start`.
        self.events.start();
        // Off the loop, so the first frame is drawn from the empty state
        // instead of behind three file scans.
        self.spawn_load();
        while self.running {
            // A frame rebuilds the calendar's event store from ninety dates
            // and re-derives the week's bar labels, so it is only worth paying
            // for when something actually changed.
            if self.dirty {
                terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
                self.dirty = false;
            }
            match self.events.next().await.context("couldn't read events")? {
                Event::Tick => self.tick(),
                // Nothing in the app changed, but the frame on screen was laid
                // out for the old size.
                Event::Crossterm(CrosstermEvent::Resize(..)) => self.dirty = true,
                Event::Crossterm(CrosstermEvent::Key(key_event))
                    if key_event.kind == KeyEventKind::Press =>
                {
                    self.handle_key_events(key_event)?;
                }
                Event::Crossterm(_) => {}
                Event::App(app_event) => {
                    if let Err(e) = self.handle_app_event(app_event, &mut terminal).await {
                        tracing::warn!("Failed to handle app event: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    /// Apply `app_event`, handling the two events that need the runtime.
    ///
    /// Everything else is delegated to [`App::apply_sync_event`], which is the
    /// single place application state changes in response to an event.
    ///
    /// **There is deliberately no `_` arm**, for the same reason
    /// [`changes_key_routing`] has none: an event that needs to await or spawn
    /// but is only listed in [`App::apply_sync_event`]'s do-nothing arm would
    /// be silently dropped by both functions, with nothing failing.
    ///
    /// Generic over the backend rather than taking a [`DefaultTerminal`], so a
    /// test can drive it against a `TestBackend` — which is what pins the
    /// `dirty` set below, the one this function's own two arms depend on.
    /// `App::run` still passes the real terminal.
    pub async fn handle_app_event<B: Backend>(
        &mut self,
        app_event: AppEvent,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        // Set here as well as in `apply_sync_event`, because the two arms this
        // function keeps for itself never reach it: `ReloadFromDisk` flips
        // `loading`, and `Edit` tears the terminal down and clears it. Setting
        // it once at the dispatch point is what stops an arm added later from
        // quietly freezing the screen.
        self.dirty = true;
        match app_event {
            // Returns immediately, but `tokio::spawn` still needs the runtime
            // this loop is running on.
            AppEvent::ReloadFromDisk => self.spawn_load(),
            AppEvent::Edit => {
                self.run_editor(terminal).await?;
                self.events.send(AppEvent::ReloadFromDisk);
            }
            e @ (AppEvent::ToggleZoomBar
            | AppEvent::ToggleHelp
            | AppEvent::CloseOverlay
            | AppEvent::Today
            | AppEvent::NextDate
            | AppEvent::PreviousDate
            | AppEvent::NextProject
            | AppEvent::PreviousProject
            | AppEvent::FirstProject
            | AppEvent::LastProject
            | AppEvent::CopyNotes
            | AppEvent::DataLoaded(..)
            | AppEvent::LoadFailed(..)
            | AppEvent::Quit) => self.apply_sync_event(e),
        }

        Ok(())
    }

    /// The part of event handling that does not await.
    ///
    /// Keeping it separate from [`App::handle_app_event`] is what lets a test
    /// send a key and then assert on the resulting state without standing up
    /// the terminal or the event loop; see `App::drain_pending_events`.
    pub fn apply_sync_event(&mut self, app_event: AppEvent) {
        // Centrally rather than per arm: every variant below is a state change
        // the screen has to catch up with, and the handful that occasionally
        // change nothing — a superseded load — cost one redundant frame, which
        // is the cheap side of the trade.
        self.dirty = true;
        match app_event {
            AppEvent::ToggleZoomBar => self.toggle_zoom_bar(),
            AppEvent::ToggleHelp => self.toggle_help(),
            AppEvent::CloseOverlay => self.overlay = None,
            AppEvent::Today => self.go_to_date(today()),
            AppEvent::NextDate => {
                self.go_to_date(self.active_date.next_day().unwrap_or(self.active_date));
            }
            AppEvent::PreviousDate => {
                self.go_to_date(self.active_date.previous_day().unwrap_or(self.active_date));
            }
            AppEvent::Quit => self.quit(),
            // Latest-wins: a load only lands while it is still the current
            // one. Holding `h` starts a load per key press, and the earlier
            // ones must not overwrite the date the user stopped on.
            AppEvent::DataLoaded(generation, payload) if generation == self.load_gen => {
                self.loading = false;
                self.apply_payload(*payload);
            }
            AppEvent::LoadFailed(generation, message) if generation == self.load_gen => {
                self.loading = false;
                tracing::warn!("load failed: {message}");
                self.set_status(format!("Load failed: {message}"));
            }
            // Superseded: a newer load is still in flight, so `loading` stays
            // set and the stale results are dropped.
            AppEvent::DataLoaded(..) | AppEvent::LoadFailed(..) => {}
            // The day view's project list owns these. They normally never
            // reach the queue — `handle_mode_key` hands them straight to the
            // widget — but a day with no list lets them fall through.
            AppEvent::NextProject
            | AppEvent::PreviousProject
            | AppEvent::FirstProject
            | AppEvent::LastProject
            | AppEvent::CopyNotes => {
                if let Some(widget) = &mut self.project_list_widget {
                    widget.apply(&app_event);
                }
            }
            // `handle_app_event` owns these: `Edit` awaits the editor, and
            // `ReloadFromDisk` spawns a load, which needs a runtime.
            //
            // Adding a variant here alone is not enough — list it in
            // `handle_app_event`'s alternation too, or it is dropped in both
            // places and no test fails.
            AppEvent::ReloadFromDisk | AppEvent::Edit => {}
        }
    }

    /// Drain everything already queued through [`App::apply_sync_event`].
    #[cfg(test)]
    pub fn drain_pending_events(&mut self) {
        while let Some(event) = self.events.try_next() {
            if let Event::App(app_event) = event {
                self.apply_sync_event(app_event);
            }
        }
    }

    /// Open the active date's week full screen, or go back to the day view.
    fn toggle_zoom_bar(&mut self) {
        self.mode = if self.mode == Mode::ZoomedWeek {
            Mode::Day
        } else {
            Mode::ZoomedWeek
        };
    }

    /// Show the help popup, or dismiss it if it is already up.
    fn toggle_help(&mut self) {
        self.overlay = if self.overlay == Some(Overlay::Help) {
            None
        } else {
            Some(Overlay::Help)
        };
    }

    /// Move to `date` and queue a reload of its data.
    fn go_to_date(&mut self, date: Date) {
        self.active_date = date;
        self.week_dates = get_week_dates(&date, self.ctx.week_start_day);
        self.events.send(AppEvent::ReloadFromDisk);
    }

    pub async fn run_editor<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Pause event polling to prevent interference with editor
        self.events.pause();

        // Stop the TUI completely before opening editor
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;

        // Create the file if it doesn't exist and get the path
        let file_path = self
            .data_svc
            .create_day_file_if_not_exists(&self.active_date)
            .await?;

        // Open the file in the user's editor
        if let Err(e) = open_in_editor(&file_path) {
            eprintln!("Failed to open editor: {}", e);
        }

        // Invalidate cache since we just edited the file
        self.data_svc.invalidate_date(&self.active_date).await;
        // The edit may have added time to an empty day or taken the last of it
        // away, either of which moves a calendar marker.
        self.month_memo.clear();

        // Restore the TUI after editor exits
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;

        // Resume event polling
        self.events.resume();
        Ok(())
    }

    /// Start loading the active date in the background, superseding whatever
    /// load is already in flight.
    ///
    /// The event loop keeps drawing and reading keys while the three file
    /// scans run, so holding `h` or `l` scrubs dates instead of freezing.
    /// Each load is stamped with a generation and reports back as
    /// [`AppEvent::DataLoaded`] or [`AppEvent::LoadFailed`]; only the newest
    /// generation is applied, so the date the user stopped on wins however
    /// the scans happen to interleave.
    ///
    /// The superseded load is aborted rather than left to finish: the
    /// generation guard drops its *result*, but only aborting bounds its
    /// *work*. A load fans out to a task per populated date in the calendar's
    /// window plus one per day of the week, and
    /// [`DataService::find_populated_dates`] keeps those children in a
    /// `JoinSet` precisely so aborting this one handle cascades to all of them
    /// — see the comment there. Without that, a held-down date key would pile
    /// them up unabortably on the one cache mutex.
    ///
    /// Must be called from inside the Tokio runtime — `App::run` and
    /// [`App::handle_app_event`] both are.
    fn spawn_load(&mut self) {
        self.load_gen += 1;
        let generation = self.load_gen;
        let tx = self.events.sender();
        let data_svc = self.data_svc.clone();
        let date = self.active_date;
        self.week_dates = get_week_dates(&date, self.ctx.week_start_day);
        let week_dates = self.week_dates;
        self.loading = true;
        let memoized = match self.month_scan_needed(date) {
            // A month the calendar has not scanned yet: the load does it.
            Some(_) => None,
            // Already scanned, and neither an edit nor an `r` has dropped it
            // since, so the ninety-day window would return this same list.
            None => self.month_memo.get(&month_key(date)).cloned(),
        };

        let handle = tokio::spawn(async move {
            match load_payload(&data_svc, date, &week_dates, memoized).await {
                Ok(payload) => tx.send(AppEvent::DataLoaded(generation, Box::new(payload))),
                Err(e) => tx.send(AppEvent::LoadFailed(generation, e.to_string())),
            }
        });
        if let Some(superseded) = self.load_task.replace(handle) {
            abort_superseded_load(superseded);
        }
    }

    /// Install a completed load. The day, the calendar and the bar chart move
    /// together, so no frame shows two different dates at once.
    fn apply_payload(&mut self, payload: LoadPayload) {
        let LoadPayload {
            day,
            populated,
            weekly,
        } = payload;
        self.set_day_data(day);
        // The generation guard has already established that this payload is
        // the active date's: every date change goes through `spawn_load`,
        // which bumps `load_gen`. So the month on screen is the month these
        // markers were scanned for.
        self.month_memo
            .insert(month_key(self.active_date), populated.clone());
        self.populated_dates = populated;
        self.weekly_data = weekly;
    }

    /// The month key `date` needs a populated-dates scan for, or `None` when
    /// [`App::month_memo`] already holds that month's markers.
    ///
    /// One entry covers the displayed month and the one either side of it —
    /// the whole window the calendar draws — which is why moving to another
    /// date in the same month needs no scan at all. The scan is the expensive
    /// half of a load, so this is the difference between an arrow key costing
    /// a directory listing and costing nothing.
    fn month_scan_needed(&self, date: Date) -> Option<(i32, u8)> {
        let key = month_key(date);
        (!self.month_memo.contains_key(&key)).then_some(key)
    }

    /// Record a one-line message for the user.
    ///
    /// Task 12 gives `App` a status field and a footer to draw it in; until
    /// then the message only reaches the log, which is enough for the call
    /// sites to be written once and left alone.
    fn set_status(&mut self, message: String) {
        tracing::debug!("status: {message}");
    }

    /// Offer `key_event` to each key layer in turn, outermost first.
    ///
    /// 1. The overlay, when one is open. Overlays are modal, so a key it does
    ///    not handle is swallowed here rather than falling through to the
    ///    view it covers.
    /// 2. The active [`Mode`], which owns the keys belonging to whatever is
    ///    on screen.
    /// 3. The global bindings, which mean the same thing everywhere.
    ///
    /// A key some layer acted on marks the app `dirty`; one no layer wanted
    /// leaves the screen exactly as it was, and must not cost a frame.
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<()> {
        let handled = if self.overlay.is_some() {
            // Overlays are modal, so a key the overlay did not handle stops
            // here rather than falling through to the view it covers.
            self.handle_overlay_key(key_event)
        } else {
            match self.handle_mode_key(key_event) {
                Handled::Ignored => self.handle_global_key(key_event),
                stopped => stopped,
            }
        };

        if !matches!(handled, Handled::Ignored) {
            self.dirty = true;
        }
        if let Handled::Emit(app_event) = handled {
            self.queue_or_apply(app_event);
        }
        Ok(())
    }

    /// Queue `app_event`, unless it decides which layer sees the next key.
    ///
    /// Keys and application events share one channel, so a key the user has
    /// already typed is queued *ahead* of anything the previous key emitted.
    /// Opening and closing an overlay therefore have to happen here rather
    /// than on the way back out of the queue: otherwise `?` followed quickly
    /// by `j` would move the project list behind the popup that is about to
    /// open.
    fn queue_or_apply(&mut self, app_event: AppEvent) {
        // `r` is the only key bound straight to a reload, and it means "read
        // the disk again" — so it has to drop the calendar's month memo too,
        // or the one gesture a user has for picking up an outside edit would
        // be answered from it. Moving between dates emits `NextDate` /
        // `PreviousDate` and reaches `ReloadFromDisk` through `go_to_date`
        // instead, which is exactly what keeps arrow keys off the scan.
        if matches!(app_event, AppEvent::ReloadFromDisk) {
            self.month_memo.clear();
        }
        if changes_key_routing(&app_event) {
            self.apply_sync_event(app_event);
        } else {
            self.events.send(app_event);
        }
    }

    /// The overlay layer: the only layer that sees a key while one is open.
    ///
    /// Returns [`Handled::Ignored`] when there is no overlay, which the caller
    /// never asks for; every other answer stops the key here.
    fn handle_overlay_key(&mut self, key_event: KeyEvent) -> Handled {
        // Raw mode delivers Ctrl-C as a key rather than a signal, so an
        // overlay that swallowed it would leave the user unable to quit
        // without first working out how to dismiss the popup.
        if is_ctrl_c(key_event) {
            return Handled::Emit(AppEvent::Quit);
        }
        match self.overlay {
            Some(Overlay::Help) => {
                if keymap::closes_overlay(key_event) {
                    Handled::Emit(AppEvent::CloseOverlay)
                } else {
                    Handled::Consumed
                }
            }
            // Task 17 gives the prompt its own editing keys; until then it is
            // unreachable, and swallowing is the safe answer either way.
            Some(Overlay::DatePrompt(_)) => Handled::Consumed,
            None => Handled::Ignored,
        }
    }

    /// The mode layer: keys that belong to whatever view is on screen.
    ///
    /// The key is resolved against the one binding table and the resulting
    /// [`AppEvent`] handed to the mode's widget, which matches on the event
    /// rather than on the key. That is what keeps the keymap in
    /// [`keymap::BINDINGS`] instead of growing a second private copy here.
    fn handle_mode_key(&mut self, key_event: KeyEvent) -> Handled {
        let Some(binding) = keymap::lookup(key_event, self.mode) else {
            return Handled::Ignored;
        };
        match self.mode {
            Mode::Day => match &mut self.project_list_widget {
                Some(widget) => widget.apply(&binding.event),
                // Nothing to navigate; the key belongs to the layer behind.
                None => Handled::Ignored,
            },
            // Tasks 16 and 20 give these modes keys of their own.
            Mode::Week | Mode::ZoomedWeek | Mode::RawFile => Handled::Ignored,
        }
    }

    /// The global layer: whatever the mode did not claim, straight from the
    /// binding table.
    ///
    /// Answers rather than acts, like the two layers above it, so
    /// [`App::handle_key_events`] has one place to queue from and one place to
    /// decide whether the key earned a redraw.
    fn handle_global_key(&self, key_event: KeyEvent) -> Handled {
        if is_ctrl_c(key_event) {
            return Handled::Emit(AppEvent::Quit);
        }
        match keymap::lookup(key_event, self.mode) {
            Some(binding) => Handled::Emit(binding.event.clone()),
            None => Handled::Ignored,
        }
    }

    /// Handles the tick event of the terminal.
    ///
    /// The tick event is where you can update the state of your application with any logic that
    /// needs to be updated at a fixed frame rate. E.g. polling a server, updating an animation.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Install a freshly loaded day, dropping the project list when the day
    /// holds no projects so the "no data" placeholder renders instead.
    fn set_day_data(&mut self, data: Option<TimeTrackingData>) {
        match data {
            Some(data) if !data.projects.is_empty() => {
                self.project_list_widget = Some(ProjectListWidget::new(&data, &self.ctx.theme));
                self.data = Some(data);
            }
            _ => {
                self.data = None;
                self.project_list_widget = None;
            }
        }
    }
}

/// Read everything one frame needs for `date`: its day, the calendar markers
/// around it, and its week's minutes.
///
/// A free function rather than a method so it can run in a spawned task,
/// holding only a cloned [`DataService`] instead of borrowing the [`App`].
async fn load_payload(
    data_svc: &DataService,
    date: Date,
    week_dates: &[Date; 7],
    memoized: Option<Vec<Date>>,
) -> Result<LoadPayload> {
    // The calendar scans the previous, current and next month, so paging a
    // month either way already has its markers.
    let current_month = date.replace_day(1).context("could not set day to 1")?;
    let start_date = month_offset(current_month, -1).context("could not compute previous month")?;
    let next_month = month_offset(current_month, 1).context("could not compute next month")?;
    let end_date = next_month
        .replace_day(next_month.month().length(next_month.year()))
        .unwrap_or(next_month);

    // Markers the caller has already scanned for this month stand in for the
    // scan entirely; see `App::month_memo`.
    let populated = async {
        match memoized {
            Some(dates) => Ok(dates),
            None => data_svc.find_populated_dates(start_date, end_date).await,
        }
    };

    // All three concurrently; the day file is usually cached, the two scans
    // are not.
    let (day, populated, weekly) = tokio::join!(
        data_svc.parse_day(&date),
        populated,
        data_svc.get_weekly_data(week_dates),
    );

    Ok(LoadPayload {
        day: day.context("Parsing the day")?,
        populated: populated.context("Finding populated dates")?,
        weekly: weekly.context("Loading weekly data")?,
    })
}

/// Abort a load a newer one has superseded, reporting a panic it may already
/// have hit.
///
/// Nothing ever awaits a load's `JoinHandle` — results come back through the
/// event queue instead — so a panic inside one would disappear along with the
/// handle. Reaping it in a detached task keeps [`App::spawn_load`] synchronous
/// while still putting the bug in the log. A clean cancellation is the
/// expected outcome and says nothing.
fn abort_superseded_load(handle: JoinHandle<()>) {
    handle.abort();
    tokio::spawn(async move {
        if let Err(e) = handle.await
            && e.is_panic()
        {
            tracing::error!("a background load panicked: {e}");
        }
    });
}

/// Does applying `app_event` change which layer sees the next key, or what
/// that layer makes of it?
///
/// Two axes qualify, and they are the reason [`App::queue_or_apply`] exists.
/// Opening or closing an [`Overlay`] changes *which* layer reads the next
/// key. Changing [`Mode`] changes what the key *means*, because
/// [`keymap::lookup`] is keyed by mode — a key already queued behind `f`
/// would otherwise be resolved against the view the user has just left.
/// Both are pure state changes, so applying them straight away is safe.
///
/// **There is deliberately no `_` arm.** Getting this wrong is silent: the
/// app still works, a key typed quickly after another just lands on the
/// wrong layer. An exhaustive match turns "someone added an event and did
/// not think about routing" from a bug into a compile error, so please
/// answer for the new variant rather than tidying this back into a
/// `matches!`.
fn changes_key_routing(app_event: &AppEvent) -> bool {
    match app_event {
        // Changes which layer reads the next key, or what it resolves to.
        AppEvent::ToggleHelp | AppEvent::CloseOverlay | AppEvent::ToggleZoomBar => true,
        // Leaves the active mode and overlay exactly as they were.
        AppEvent::NextProject
        | AppEvent::PreviousProject
        | AppEvent::FirstProject
        | AppEvent::LastProject
        | AppEvent::CopyNotes
        | AppEvent::Edit
        | AppEvent::NextDate
        | AppEvent::PreviousDate
        | AppEvent::ReloadFromDisk
        | AppEvent::Today
        // Never reach this function at all — a load reports back, it is not
        // emitted by a key — and they touch neither mode nor overlay.
        | AppEvent::DataLoaded(..)
        | AppEvent::LoadFailed(..)
        | AppEvent::Quit => false,
    }
}

/// Ctrl-C, which raw mode delivers as a key event rather than as a signal.
fn is_ctrl_c(key_event: KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::CONTROL
        && matches!(key_event.code, KeyCode::Char('c' | 'C'))
}

/// The `(year, month)` [`App::month_memo`] keys a date's calendar under.
fn month_key(date: Date) -> (i32, u8) {
    (date.year(), u8::from(date.month()))
}

/// Today's date in the local timezone, falling back to UTC.
fn today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

/// Return the first day of the month `offset` months from `date` (negative = previous).
fn month_offset(date: Date, offset: i32) -> Result<Date> {
    if offset == 0 {
        return Ok(date);
    }
    // Try the simple replace_month path first (works when the month cycle doesn't cross a year)
    let next_month = if offset > 0 {
        date.month().next()
    } else {
        date.month().previous()
    };
    match date.replace_month(next_month) {
        Ok(d) => Ok(d),
        Err(_) => {
            // Month wrapped around the year boundary; adjust the year too
            let new_year = if offset > 0 {
                date.year().checked_add(1)
            } else {
                date.year().checked_sub(1)
            }
            .context("year out of range computing adjacent month")?;
            let boundary_month = if offset > 0 {
                time::Month::January
            } else {
                time::Month::December
            };
            date.replace_year(new_year)
                .context("replace_year")?
                .replace_month(boundary_month)
                .context("replace_month at year boundary")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testing::{fixture_date, fixture_day};
    use ratatui::{backend::TestBackend, crossterm::event::KeyCode};
    use std::time::Duration;
    use time::macros::date;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn payload_with(populated: Vec<Date>) -> Box<LoadPayload> {
        Box::new(LoadPayload {
            day: None,
            populated,
            weekly: HashMap::new(),
        })
    }

    /// Wait for the next app event the queue produces.
    ///
    /// `App::run` would await `EventHandler::next`, but that needs the
    /// crossterm poller started — which a test must not do, since it would
    /// read the real tty. Polling the same queue is equivalent for a load
    /// that reports back from a spawned task.
    async fn next_app_event(app: &mut App) -> AppEvent {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match app.events.try_next() {
                    Some(Event::App(app_event)) => return app_event,
                    Some(_) => {}
                    None => tokio::time::sleep(Duration::from_millis(5)).await,
                }
            }
        })
        .await
        .expect("the spawned load should report back")
    }

    fn day_app() -> App {
        App::new(TuiContext::for_test())
            .with_active_date(fixture_date())
            .with_data(fixture_day())
    }

    fn selection(app: &App) -> Option<usize> {
        app.project_list_widget.as_ref()?.selected_item()
    }

    /// Keys and application events share one channel, so a second key the
    /// user has already typed is queued ahead of anything the first emitted.
    /// An overlay that only opened once the queue drained would let that
    /// second key through to the view behind the popup.
    #[test]
    fn a_key_typed_straight_after_the_help_key_lands_on_the_popup() {
        let mut app = day_app();

        app.handle_key_events(key('?')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.overlay, Some(Overlay::Help));
        assert_eq!(
            selection(&app),
            Some(0),
            "j must not move the list behind the popup"
        );
    }

    /// The other half of the same rule: closing is synchronous too, so the
    /// next key reaches the view rather than being swallowed by a popup that
    /// is already gone from the user's point of view.
    #[test]
    fn a_key_typed_straight_after_closing_the_popup_reaches_the_view() {
        let mut app = day_app();
        app.overlay = Some(Overlay::Help);

        app.handle_key_events(key('q')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert!(app.overlay.is_none());
        assert_eq!(selection(&app), Some(1));
    }

    /// The same rule on the mode axis. `keymap::lookup` is keyed by mode, so
    /// a mode change that waited for the queue would resolve the key behind
    /// it against the view the user has just left.
    #[test]
    fn a_key_typed_straight_after_the_zoom_key_resolves_in_the_new_mode() {
        let mut app = day_app();

        app.handle_key_events(key('f')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.mode, Mode::ZoomedWeek);
        assert_eq!(
            selection(&app),
            Some(0),
            "j is not bound in the zoomed week, so it must not move the list"
        );
    }

    /// Guards the test above from passing vacuously: zooming back out has to
    /// hand `j` to the project list again, in the same key-handling pass.
    #[test]
    fn zooming_back_out_gives_the_next_key_to_the_project_list() {
        let mut app = day_app();

        app.handle_key_events(key('f')).unwrap();
        app.handle_key_events(key('f')).unwrap();
        app.handle_key_events(key('j')).unwrap();
        app.drain_pending_events();

        assert_eq!(app.mode, Mode::Day);
        assert_eq!(selection(&app), Some(1));
    }

    /// The project-list events are only meaningful against a list; a day with
    /// no data must not panic when one reaches the queue instead of a widget.
    #[test]
    fn project_events_are_harmless_when_the_day_has_no_list() {
        let mut app = App::new(TuiContext::for_test()).with_active_date(fixture_date());
        assert!(app.project_list_widget.is_none());

        for c in ['j', 'k', 'g', 'G'] {
            app.handle_key_events(key(c)).unwrap();
        }
        app.drain_pending_events();

        assert!(app.running);
        assert_eq!(selection(&app), None);
    }

    /// `g` and `G` are separate bindings, and crossterm delivers the capital
    /// with `SHIFT` set — which a table lookup has to tolerate.
    #[test]
    fn shifted_g_jumps_to_the_last_project() {
        let mut app = day_app();

        app.handle_key_events(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT))
            .unwrap();
        app.drain_pending_events();
        assert_eq!(selection(&app), Some(2));

        app.handle_key_events(key('g')).unwrap();
        app.drain_pending_events();
        assert_eq!(selection(&app), Some(0));
    }

    /// The guard that makes off-loop loading safe. Holding `h` starts a load
    /// per key press against a different date; whichever finishes last would
    /// otherwise win, leaving the screen on a date the user scrolled past.
    #[test]
    fn stale_load_results_are_discarded() {
        let mut app = App::new(TuiContext::for_test());
        app.load_gen = 7;
        app.loading = true;

        app.apply_sync_event(AppEvent::DataLoaded(
            6,
            payload_with(vec![date!(2026 - 01 - 01)]),
        ));
        assert!(
            app.populated_dates.is_empty(),
            "generation 6 is stale and must be dropped"
        );
        assert!(app.loading, "generation 7 is still in flight");

        app.apply_sync_event(AppEvent::DataLoaded(
            7,
            payload_with(vec![date!(2026 - 02 - 02)]),
        ));
        assert_eq!(app.populated_dates, vec![date!(2026 - 02 - 02)]);
        assert!(!app.loading);
    }

    /// The same guard on the failure path: a stale error must not clear the
    /// in-flight flag, or report a failure the user has already navigated
    /// away from.
    #[test]
    fn a_stale_load_failure_is_discarded() {
        let mut app = App::new(TuiContext::for_test());
        app.load_gen = 3;
        app.loading = true;

        app.apply_sync_event(AppEvent::LoadFailed(2, "superseded".to_owned()));
        assert!(app.loading, "generation 3 is still in flight");

        app.apply_sync_event(AppEvent::LoadFailed(3, "boom".to_owned()));
        assert!(!app.loading);
    }

    /// End to end: the load runs in a spawned task and reports back through
    /// the same queue the keys arrive on, rather than being awaited inline.
    #[tokio::test]
    async fn a_spawned_load_reports_back_through_the_event_queue() {
        let dir = tempfile::tempdir().expect("temp dir");
        tokio::fs::write(
            dir.path().join("2025-06-11.md"),
            "9:00-10:30 admin\n- standup\n",
        )
        .await
        .expect("write the fixture day file");

        let ctx = TuiContext {
            data_dir: dir.path().to_path_buf(),
            ..TuiContext::for_test()
        };
        let mut app = App::new(ctx).with_active_date(fixture_date());

        app.spawn_load();
        assert!(app.loading, "the load is in flight");
        assert_eq!(app.load_gen, 1);

        let event = next_app_event(&mut app).await;
        assert!(
            matches!(event, AppEvent::DataLoaded(1, _)),
            "expected the first generation's payload, got {event:?}"
        );
        app.apply_sync_event(event);

        assert!(!app.loading);
        assert_eq!(app.data.map(|d| d.total_minutes), Some(90));
        assert_eq!(app.populated_dates, vec![fixture_date()]);
        assert_eq!(app.weekly_data.get(&fixture_date()).copied(), Some(90));
    }

    /// The loop draws only when `dirty` is set, so a key that changes what is
    /// on screen without setting it renders as a frozen UI — no error, no
    /// panic, just a screen that stops updating.
    #[test]
    fn a_handled_key_marks_the_app_dirty() {
        let mut app = day_app();
        app.dirty = false;

        app.handle_key_events(key('l')).unwrap();

        assert!(app.dirty, "a handled key must request a redraw");
    }

    /// The mode layer answers `Consumed` rather than emitting an event, so
    /// nothing further downstream would ever set the flag on its behalf.
    #[test]
    fn a_key_the_project_list_consumed_marks_the_app_dirty() {
        let mut app = day_app();
        app.dirty = false;

        app.handle_key_events(key('j')).unwrap();

        assert_eq!(selection(&app), Some(1));
        assert!(app.dirty, "moving the selection has to repaint");
    }

    /// The other half of the rule, and the whole point of the task: a key no
    /// layer wanted changed nothing, so repainting for it would put the cost
    /// of the old unconditional draw straight back under another name.
    #[test]
    fn an_unbound_key_does_not_mark_the_app_dirty() {
        let mut app = day_app();
        app.dirty = false;

        app.handle_key_events(key('\u{1}')).unwrap();

        assert!(!app.dirty, "an unbound key must not force a repaint");
    }

    /// The second seam: everything that reaches `apply_sync_event` is a state
    /// change, including the loads that arrive with no key behind them.
    #[test]
    fn applying_a_sync_event_marks_the_app_dirty() {
        let mut app = day_app();

        app.dirty = false;
        app.apply_sync_event(AppEvent::ToggleHelp);
        assert!(app.dirty, "opening the help popup has to repaint");

        app.dirty = false;
        app.apply_sync_event(AppEvent::DataLoaded(
            app.load_gen,
            payload_with(vec![date!(2026 - 03 - 03)]),
        ));
        assert!(app.dirty, "a landed load has to repaint");
    }

    /// The seam item (b) of the brief named. `ReloadFromDisk` and `Edit` are
    /// the two events `handle_app_event` keeps for itself, so neither reaches
    /// `apply_sync_event` — and `spawn_load` writes `loading` and `week_dates`
    /// without marking anything. The central set at the top of
    /// `handle_app_event` is the only thing repainting for them, which makes
    /// it load-bearing today rather than merely defensive.
    #[tokio::test]
    async fn an_event_dispatched_off_the_queue_marks_the_app_dirty() {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test backend");
        let mut app = day_app();
        app.dirty = false;

        app.handle_app_event(AppEvent::ReloadFromDisk, &mut terminal)
            .await
            .unwrap();

        assert!(app.loading, "the reload is in flight");
        assert!(
            app.dirty,
            "an event dispatched off the queue must request a redraw"
        );
    }

    /// The generation guard drops a superseded load's *result*; aborting is
    /// what bounds its *work*. The load's children live in `DataService`'s
    /// `JoinSet`s, so aborting this one outer handle cascades to all of them —
    /// without it a held-down date key leaves them contending on the single
    /// cache mutex long after the user stopped scrubbing.
    #[tokio::test]
    async fn aborting_a_superseded_load_stops_it() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let _ = tx.send(());
        });
        let superseded = handle.abort_handle();

        abort_superseded_load(handle);

        tokio::time::timeout(Duration::from_secs(5), async {
            while !superseded.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the superseded load should stop rather than run on");
        assert!(
            rx.await.is_err(),
            "the aborted load must never reach its work"
        );
    }

    /// ... and `spawn_load` is what hands the old one over: without that call
    /// the abort above is dead code and a scrubbed-past load runs out in full.
    #[tokio::test]
    async fn a_new_load_aborts_the_one_it_supersedes() {
        let mut app = App::new(TuiContext::for_test());

        app.spawn_load();
        let superseded = app
            .load_task
            .as_ref()
            .expect("a load is in flight")
            .abort_handle();
        app.spawn_load();

        // One turn of the current-thread runtime is enough to drop an aborted
        // task, and nowhere near enough for a live load to finish its three
        // file scans — so this distinguishes the two without a sleep.
        tokio::task::yield_now().await;
        assert!(
            superseded.is_finished(),
            "the superseded load must be aborted, not left to run out"
        );
        assert_ne!(
            superseded.id(),
            app.load_task.as_ref().expect("a load is in flight").id(),
            "the newer load must hold the slot"
        );
    }

    /// Scrubbing dates touches no disk on the event loop: each key moves the
    /// date and leaves a reload on the queue for `handle_app_event` to spawn.
    #[test]
    fn scrubbing_dates_only_queues_reloads() {
        let mut app = day_app();

        for _ in 0..3 {
            app.handle_key_events(key('l')).unwrap();
        }
        app.drain_pending_events();

        assert_eq!(app.active_date, date!(2025 - 06 - 14));
        assert_eq!(app.load_gen, 0, "no load may start on the event loop");
        assert!(!app.loading);
    }

    /// The predicate the memo turns on: within a month it has already
    /// scanned there is nothing to do, and a month it has not is a scan.
    #[test]
    fn same_month_navigation_reuses_the_memo() {
        let mut app = App::new(TuiContext::for_test());
        app.month_memo
            .insert((2026, 8), vec![date!(2026 - 08 - 24)]);

        assert!(app.month_scan_needed(date!(2026 - 08 - 25)).is_none());
        assert_eq!(
            app.month_scan_needed(date!(2026 - 09 - 01)),
            Some((2026, 9))
        );
    }

    /// A day directory that the app can then read from, holding one populated
    /// day. The `TempDir` must outlive the app or the directory goes away.
    async fn app_on_a_seeded_dir() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        tokio::fs::write(dir.path().join("2025-06-11.md"), "9:00-10:30 admin\n")
            .await
            .expect("write the fixture day file");
        let ctx = TuiContext {
            data_dir: dir.path().to_path_buf(),
            ..TuiContext::for_test()
        };
        (App::new(ctx).with_active_date(fixture_date()), dir)
    }

    /// Run one load to completion, the way `App::run` would.
    ///
    /// Skips past anything else already queued — pressing `r` leaves a
    /// `ReloadFromDisk` behind, which the real loop turns into the spawn this
    /// helper makes by hand.
    async fn load_once(app: &mut App) {
        app.spawn_load();
        loop {
            let event = next_app_event(app).await;
            let landed = matches!(event, AppEvent::DataLoaded(..) | AppEvent::LoadFailed(..));
            app.apply_sync_event(event);
            if landed {
                return;
            }
        }
    }

    /// The whole point of the memo, asserted on behaviour rather than on a
    /// counter: a file that appears after the month has been scanned must not
    /// show up on a same-month date change, because no scan happened.
    #[tokio::test]
    async fn a_same_month_date_change_reuses_the_memoized_markers() {
        let (mut app, dir) = app_on_a_seeded_dir().await;

        load_once(&mut app).await;
        assert_eq!(app.populated_dates, vec![fixture_date()]);

        // A second populated day appears behind the memo's back.
        tokio::fs::write(dir.path().join("2025-06-12.md"), "9:00-10:30 admin\n")
            .await
            .expect("write the second day file");
        app.active_date = date!(2025 - 06 - 13);
        load_once(&mut app).await;

        assert_eq!(
            app.populated_dates,
            vec![fixture_date()],
            "a same-month date change must not rescan the ninety-day window"
        );
    }

    /// ... and the memo has to be droppable, or it is just a stale cache. `r`
    /// means "read the disk again", so it clears the markers as well.
    #[tokio::test]
    async fn an_explicit_reload_drops_the_memo_and_rescans() {
        let (mut app, dir) = app_on_a_seeded_dir().await;

        load_once(&mut app).await;
        tokio::fs::write(dir.path().join("2025-06-12.md"), "9:00-10:30 admin\n")
            .await
            .expect("write the second day file");

        app.handle_key_events(key('r')).unwrap();
        assert!(
            app.month_memo.is_empty(),
            "an explicit reload must drop the month markers"
        );
        load_once(&mut app).await;

        assert_eq!(
            app.populated_dates,
            vec![fixture_date(), date!(2025 - 06 - 12)]
        );
    }

    /// Moving between dates emits `NextDate`/`PreviousDate` and reaches
    /// `ReloadFromDisk` only through `go_to_date` — which is exactly what
    /// keeps arrow keys off the scan. If navigation ever emitted the reload
    /// event directly, the memo would be cleared on every key press and this
    /// task would silently undo itself.
    #[test]
    fn moving_between_dates_does_not_drop_the_memo() {
        let mut app = day_app();
        app.month_memo.insert((2025, 6), vec![fixture_date()]);

        app.handle_key_events(key('l')).unwrap();
        app.drain_pending_events();

        assert!(app.month_memo.contains_key(&(2025, 6)));
    }
}
