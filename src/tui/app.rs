use crate::{Config, DefaultDisplayFormatter, DisplayFormatter, display::read_day};

use super::event::{AppEvent, Event, EventHandler};
use anyhow::{Context, Result};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::List,
};
use time::{Date, OffsetDateTime};
use time_tracking_parser::TimeTrackingData;

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Counter.
    pub counter: u8,
    pub selected_item: u8,
    pub config: Config,
    pub active_date: Date,
    pub formatter: Box<dyn DisplayFormatter>,
    /// Event handler.
    pub events: EventHandler,
    pub data: Option<TimeTrackingData>,
    pub day_summary: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            counter: 0,
            selected_item: 0,
            active_date: OffsetDateTime::now_utc().date(),
            events: EventHandler::new(),
            formatter: Box::new(DefaultDisplayFormatter),
            config: Config::default(),
            day_summary: None,
            data: None,
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
        self.load_data_for_active_date().await?;
        while self.running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
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
                    AppEvent::NextDate => {
                        self.active_date = self.active_date.next_day().unwrap_or(self.active_date);
                        self.load_data_for_active_date().await?;
                        self.selected_item = 0;
                    }
                    AppEvent::PreviousDate => {
                        self.active_date =
                            self.active_date.previous_day().unwrap_or(self.active_date);
                        self.load_data_for_active_date().await?;
                        self.selected_item = 0;
                    }
                    AppEvent::NextItem => {
                        let _ = self.selected_item.saturating_add(1);
                    }
                    AppEvent::PreviousItem => {
                        let _ = self.selected_item.saturating_sub(1);
                    }
                    AppEvent::Increment => self.increment_counter(),
                    AppEvent::Decrement => self.decrement_counter(),
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
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
            self.data = Some(data);
            self.day_summary = Some(self.formatter.day_summary(
                &content,
                "",
                self.config.prefix.as_deref(),
                self.config.suffix.as_deref(),
            ));
        } else {
            self.data = None;
            self.day_summary = None;
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Right => {
                self.events.send(AppEvent::Increment);
                self.events.send(AppEvent::NextDate);
            }
            KeyCode::Left => {
                self.events.send(AppEvent::Decrement);
                self.events.send(AppEvent::PreviousDate);
            }
            KeyCode::Char('j') | KeyCode::Down => self.events.send(AppEvent::NextItem),
            KeyCode::Char('k') | KeyCode::Up => self.events.send(AppEvent::PreviousItem),
            // Other handlers you could add here.
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

    pub fn increment_counter(&mut self) {
        self.counter = self.counter.saturating_add(1);
    }

    pub fn decrement_counter(&mut self) {
        self.counter = self.counter.saturating_sub(1);
    }
}
