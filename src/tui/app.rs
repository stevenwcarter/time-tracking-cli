use crate::time_utils::{get_week_dates, parse_weekday};
use std::{collections::HashMap, io::stdout};

use crate::{Config, DataService, editor::open_in_editor};

use super::{
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
    /// Current active date
    pub active_date: Date,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            zoom_bar: false,
            show_help: false,
            active_date: OffsetDateTime::now_local()
                    .unwrap_or_else(|_| OffsetDateTime::now_utc())
                    .date(),
            events: EventHandler::new(),
            data: None,
            project_list_widget: None,
            populated_dates: Vec::new(),
            weekly_data: HashMap::new(),
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new() -> Self {
        Self {
            active_date: Config::get().date,
            ..Default::default()
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
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
                self.active_date = OffsetDateTime::now_local()
                    .unwrap_or_else(|_| OffsetDateTime::now_utc())
                    .date();
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
        let file_path = DataService::get()
            .create_day_file_if_not_exists(&self.active_date)
            .await?;

        // Open the file in the user's editor
        if let Err(e) = open_in_editor(&file_path) {
            eprintln!("Failed to open editor: {}", e);
        }

        // Invalidate cache since we just edited the file
        DataService::get().invalidate_date(&self.active_date).await;

        // Restore the TUI after editor exits
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;

        // Resume event polling
        self.events.resume();
        Ok(())
    }

    pub async fn load_data_for_active_date(&mut self) -> Result<()> {
        if let Some(data) = DataService::get().parse_day(&self.active_date).await? {
            // Create project list widget with the data
            if !data.projects.is_empty() {
                self.project_list_widget = Some(ProjectListWidget::new(&data));
                self.data = Some(data);
            } else {
                self.data = None;
                self.project_list_widget = None;
            }
        } else {
            self.data = None;
            self.project_list_widget = None;
        }

        self.find_populated_dates()
            .await
            .context("Finding populated dates")?;

        self.load_weekly_data()
            .await
            .context("Loading weekly data")?;

        Ok(())
    }

    pub async fn load_weekly_data(&mut self) -> Result<()> {
        let week_start_day = parse_weekday(Config::get().get_week_start_day())
            .context("Could not parse week start day")?;
        let week_dates = get_week_dates(&self.active_date, week_start_day);

        self.weekly_data = DataService::get().get_weekly_data(&week_dates).await?;

        Ok(())
    }

    pub async fn find_populated_dates(&mut self) -> Result<()> {
        // Get current month, previous month, and next month
        let current_month = self.active_date.replace_day(1).unwrap();
        let prev_month = current_month
            .replace_month(current_month.month().previous())
            .unwrap_or_else(|_| {
                current_month
                    .replace_year(current_month.year() - 1)
                    .unwrap()
                    .replace_month(time::Month::December)
                    .unwrap()
            });
        let next_month = current_month
            .replace_month(current_month.month().next())
            .unwrap_or_else(|_| {
                current_month
                    .replace_year(current_month.year() + 1)
                    .unwrap()
                    .replace_month(time::Month::January)
                    .unwrap()
            });

        // Calculate start and end dates
        let start_date = prev_month;
        let end_date = next_month
            .replace_day(next_month.month().length(next_month.year()))
            .unwrap_or(next_month);

        self.populated_dates = DataService::get()
            .find_populated_dates(start_date, end_date)
            .await?;

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
}
