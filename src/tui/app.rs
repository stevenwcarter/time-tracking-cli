use std::{
    collections::HashMap,
    io::stdout,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{
    Config, DATE_FORMAT, DefaultDisplayFormatter, DisplayFormatter, display::read_day,
    editor::open_in_editor, file_utils::get_time_tracking_dir_with_override,
};

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
use tokio::fs;

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Is the application zoomed into the bar chart
    pub zoom_bar: bool,
    /// Current configuration
    pub config: Config,
    /// Is help popup currently being shown
    pub show_help: bool,
    /// Current active date
    pub active_date: Date,
    /// Selected formatter (TODO: implement this)
    pub formatter: Box<dyn DisplayFormatter>,
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
    /// Cache of file modification times for performance
    file_mod_times: HashMap<Date, SystemTime>,
    /// Last time populated dates were checked
    last_populated_check: Option<SystemTime>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            zoom_bar: false,
            show_help: false,
            active_date: OffsetDateTime::now_local().unwrap().date(),
            events: EventHandler::new(),
            formatter: Box::new(DefaultDisplayFormatter),
            config: Config::default(),
            data: None,
            project_list_widget: None,
            populated_dates: Vec::new(),
            weekly_data: HashMap::new(),
            file_mod_times: HashMap::new(),
            last_populated_check: None,
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new(config: &Config, formatter: Box<dyn DisplayFormatter>) -> Self {
        Self {
            active_date: config.date,
            config: config.clone(),
            formatter,
            ..Self::default()
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let _ = self.load_data_for_active_date().await;
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
                Event::App(app_event) => match app_event {
                    AppEvent::ReloadFromDisk => {
                        self.load_data_for_active_date().await?;
                    }
                    AppEvent::ToggleZoomBar => {
                        self.toggle_zoom_bar();
                    }
                    AppEvent::Edit => {
                        self.run_editor(&mut terminal)?;
                        self.events.send(AppEvent::ReloadFromDisk);
                    }
                    AppEvent::Today => {
                        self.active_date = OffsetDateTime::now_local().unwrap().date();
                        self.events.send(AppEvent::ReloadFromDisk);
                    }
                    AppEvent::ToggleHelp => self.show_help = !self.show_help,
                    AppEvent::NextDate => {
                        self.active_date = self.active_date.next_day().unwrap_or(self.active_date);
                        self.events.send(AppEvent::ReloadFromDisk);
                    }
                    AppEvent::PreviousDate => {
                        self.active_date =
                            self.active_date.previous_day().unwrap_or(self.active_date);
                        self.events.send(AppEvent::ReloadFromDisk);
                    }
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
    }

    pub fn toggle_zoom_bar(&mut self) {
        self.zoom_bar = !self.zoom_bar;
    }

    pub fn run_editor(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        // Pause event polling to prevent interference with editor
        self.events.pause();

        // Stop the TUI completely before opening editor
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;

        // Build the file path for the current date using config
        let file_path = self.get_file_path_for_active_date()?;

        // Open the file in the user's editor
        if let Err(e) = open_in_editor(&file_path) {
            eprintln!("Failed to open editor: {}", e);
        }

        // Restore the TUI after editor exits
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;

        // Resume event polling
        self.events.resume();
        Ok(())
    }

    fn get_file_path_for_active_date(&self) -> Result<PathBuf> {
        // Get the time tracking directory using config data_directory
        let time_tracking_dir =
            get_time_tracking_dir_with_override(self.config.get_data_directory())?;

        let prefix = self.config.get_prefix().unwrap_or("");
        let suffix = self.config.get_suffix().unwrap_or("");

        // Format the date as YYYY-MM-DD
        let date_str = self.active_date.format(DATE_FORMAT).unwrap();
        let filename = format!("{}{}{}.md", prefix, date_str, suffix);

        Ok(time_tracking_dir.join(filename))
    }

    pub async fn load_data_for_active_date(&mut self) -> Result<()> {
        let content = read_day(&self.active_date, &self.config)
            .await
            .context("could not read day")?;
        if let Some(content) = content {
            let data = time_tracking_parser::parse_time_tracking_data(
                &content,
                self.config.prefix.as_deref(),
                self.config.suffix.as_deref(),
            );

            // Create project list widget with the data
            if !data.projects.is_empty() {
                self.project_list_widget = Some(ProjectListWidget::new(&data));
                self.data = Some(data);
            } else {
                self.data = None;
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
        use crate::time_utils::{get_week_dates, parse_weekday};

        let week_start_day = parse_weekday(self.config.get_week_start_day())
            .context("Could not parse week start day")?;
        let week_dates = get_week_dates(&self.active_date, week_start_day);
        let time_tracking_dir =
            get_time_tracking_dir_with_override(self.config.get_data_directory())?;

        self.weekly_data.clear();

        for date in week_dates {
            let filename = format!("{}.md", date.format(&DATE_FORMAT)?);
            let file_path = time_tracking_dir.join(&filename);

            let total_minutes = if file_path.exists() {
                let content = fs::read_to_string(&file_path)
                    .await
                    .context("Reading file")?;
                let data = time_tracking_parser::parse_time_tracking_data(
                    &content,
                    self.config.get_prefix(),
                    self.config.get_suffix(),
                );
                data.total_minutes
            } else {
                0
            };

            self.weekly_data.insert(date, total_minutes);
        }

        Ok(())
    }

    pub async fn find_populated_dates(&mut self) -> Result<()> {
        let now = SystemTime::now();

        // Skip if we checked recently and no files have been modified
        // if let Some(last_check) = self.last_populated_check {
        //     if now.duration_since(last_check).unwrap_or_default().as_secs() < 5 {
        //         // Only re-scan if it's been more than 30 seconds
        //         return Ok(());
        //     }
        // }

        let time_tracking_dir =
            get_time_tracking_dir_with_override(self.config.get_data_directory())?;

        // Create directory if it doesn't exist
        if !time_tracking_dir.exists() {
            return Ok(());
        }

        let mut new_populated_dates = Vec::new();
        let mut new_mod_times = HashMap::new();

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

        // Check all dates in the three months
        for month_start in [prev_month, current_month, next_month] {
            let days_in_month = month_start.month().length(month_start.year());

            for day in 1..=days_in_month {
                if let Ok(date) = month_start.replace_day(day)
                    && let Ok(has_data) = self
                        .check_date_has_data(&date, &time_tracking_dir, &mut new_mod_times)
                        .await
                    && has_data
                {
                    new_populated_dates.push(date);
                }
            }
        }

        self.populated_dates = new_populated_dates;
        self.file_mod_times = new_mod_times;
        self.last_populated_check = Some(now);

        Ok(())
    }

    async fn check_date_has_data(
        &self,
        date: &Date,
        time_tracking_dir: &Path,
        mod_times: &mut HashMap<Date, SystemTime>,
    ) -> Result<bool> {
        let date_str = date.format(DATE_FORMAT).context("could not format date")?;
        let filename = format!("{}.md", date_str);
        let file_path = time_tracking_dir.join(filename);

        if !file_path.exists() {
            return Ok(false);
        }

        // Check file modification time for caching
        let metadata = fs::metadata(&file_path).await?;
        let mod_time = metadata.modified()?;

        // If we have a cached modification time and it hasn't changed, use cached result
        if let Some(cached_mod_time) = self.file_mod_times.get(date)
            && *cached_mod_time == mod_time
        {
            // File hasn't changed, check if we already know it has data
            return Ok(self.populated_dates.contains(date));
        }

        // Store the modification time
        mod_times.insert(*date, mod_time);

        // Read and parse the file to check for data
        let content = fs::read_to_string(&file_path).await?;
        let data = time_tracking_parser::parse_time_tracking_data(
            &content,
            self.config.get_prefix(),
            self.config.get_suffix(),
        );

        // Consider a date populated if it has projects with time > 0
        Ok(!data.projects.is_empty() && data.total_minutes > 0)
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
