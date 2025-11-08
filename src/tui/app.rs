use std::{io::stdout, path::PathBuf};

use crate::{
    Config, DefaultDisplayFormatter, DisplayFormatter, display::read_day, editor::open_in_editor,
    file_utils::get_time_tracking_dir_with_override,
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

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            show_help: false,
            active_date: OffsetDateTime::now_local().unwrap().date(),
            events: EventHandler::new(),
            formatter: Box::new(DefaultDisplayFormatter),
            config: Config::default(),
            data: None,
            project_list_widget: None,
            populated_dates: Vec::new(),
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new(config: &Config, date: Date, formatter: Box<dyn DisplayFormatter>) -> Self {
        Self {
            active_date: date,
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
        let date_str = self
            .active_date
            .format(&time::format_description::parse("[year]-[month]-[day]").unwrap())
            .unwrap();
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

        Ok(())
    }

    pub async fn find_populated_dates(&mut self) -> Result<()> {
        // TODO: Populate dates with hours for this month and the previous/next month
        // Load each date for the current month, previous month, and next month, then any that have
        // more than 0 hours should be included in self.populated_dates (which is a Vec<time::Date>)
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
