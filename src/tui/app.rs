use crate::time_utils::get_week_dates;
use std::{collections::HashMap, io::stdout};

use crate::{DataService, editor::open_in_editor};

use super::{
    context::TuiContext,
    event::{AppEvent, Event, EventHandler},
    project_list::ProjectListWidget,
};
use anyhow::{Context, Result};
use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use time::{Date, OffsetDateTime};
use time_tracking_parser::TimeTrackingData;

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Is the application zoomed into the bar chart
    pub zoom_bar: bool,
    /// Is help popup currently being shown
    pub show_help: bool,
    /// Is a load of the active date's data currently in flight
    pub loading: bool,
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
            zoom_bar: false,
            show_help: false,
            loading: false,
            active_date,
            week_dates,
            events: EventHandler::new(),
            data: None,
            project_list_widget: None,
            populated_dates: Vec::new(),
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
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // Nothing polls the terminal until now; see `EventHandler::start`.
        self.events.start();
        if let Err(e) = self.load_data_for_active_date().await {
            tracing::warn!("Failed to load data on startup: {e}");
        }
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            match self.events.next().await.context("couldn't read events")? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event)
                        if key_event.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        self.handle_key_events(key_event)?
                    }
                    _ => {}
                },
                Event::App(app_event) => {
                    if let Err(e) = self.handle_app_event(app_event, &mut terminal).await {
                        tracing::warn!("Failed to handle app event: {e}");
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn handle_app_event(
        &mut self,
        app_event: AppEvent,
        terminal: &mut DefaultTerminal,
    ) -> Result<()> {
        match app_event {
            AppEvent::ReloadFromDisk => {
                self.load_data_for_active_date().await?;
            }
            AppEvent::ToggleZoomBar => {
                self.toggle_zoom_bar();
            }
            AppEvent::Edit => {
                self.run_editor(terminal).await?;
                self.events.send(AppEvent::ReloadFromDisk);
            }
            AppEvent::Today => {
                self.active_date = today();
                self.events.send(AppEvent::ReloadFromDisk);
            }
            AppEvent::ToggleHelp => self.show_help = !self.show_help,
            AppEvent::NextDate => {
                self.active_date = self.active_date.next_day().unwrap_or(self.active_date);
                self.events.send(AppEvent::ReloadFromDisk);
            }
            AppEvent::PreviousDate => {
                self.active_date = self.active_date.previous_day().unwrap_or(self.active_date);
                self.events.send(AppEvent::ReloadFromDisk);
            }
            AppEvent::Quit => self.quit(),
        }

        Ok(())
    }

    pub fn toggle_zoom_bar(&mut self) {
        self.zoom_bar = !self.zoom_bar;
    }

    pub async fn run_editor(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
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

        // Restore the TUI after editor exits
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;

        // Resume event polling
        self.events.resume();
        Ok(())
    }

    pub async fn load_data_for_active_date(&mut self) -> Result<()> {
        let active_date = self.active_date;
        let data_svc = self.data_svc.clone();

        // Compute date ranges for the populated-dates scan (prev, current, next month)
        let current_month = active_date
            .replace_day(1)
            .context("could not set day to 1")?;
        let prev_month =
            month_offset(current_month, -1).context("could not compute previous month")?;
        let next_month = month_offset(current_month, 1).context("could not compute next month")?;
        let start_date = prev_month;
        let end_date = next_month
            .replace_day(next_month.month().length(next_month.year()))
            .unwrap_or(next_month);

        self.week_dates = get_week_dates(&active_date, self.ctx.week_start_day);
        let week_dates = self.week_dates;

        // Run all three data loads concurrently
        let (day_result, populated_result, weekly_result) = tokio::join!(
            data_svc.parse_day(&active_date),
            data_svc.find_populated_dates(start_date, end_date),
            data_svc.get_weekly_data(&week_dates),
        );

        // Apply results
        self.set_day_data(day_result?);
        self.populated_dates = populated_result.context("Finding populated dates")?;
        self.weekly_data = weekly_result.context("Loading weekly data")?;

        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<()> {
        // First try to handle project list specific events
        if let Some(ref mut widget) = self.project_list_widget
            && widget.handle_key_event(key_event)
        {
            // Event was handled by the widget, return early
            return Ok(());
        }

        // Handle app-level events
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('e') => self.events.send(AppEvent::Edit),
            KeyCode::Char('f') => self.events.send(AppEvent::ToggleZoomBar),
            KeyCode::Char('r') => self.events.send(AppEvent::ReloadFromDisk),
            KeyCode::Char('t' | 'T') => self.events.send(AppEvent::Today),
            KeyCode::Char('l') | KeyCode::Right => self.events.send(AppEvent::NextDate),
            KeyCode::Char('h') | KeyCode::Left => self.events.send(AppEvent::PreviousDate),
            KeyCode::Char('?') => self.events.send(AppEvent::ToggleHelp),
            _ => {}
        }
        Ok(())
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
