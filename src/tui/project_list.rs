#![allow(dead_code)]
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{
        Color, Modifier, Style, Stylize,
        palette::tailwind::{BLUE, GREEN, SLATE},
    },
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget,
        Widget, Wrap,
    },
};
use time_tracking_parser::{TimeTrackingData, format_time, format_time_option};
const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

#[derive(Default, Debug)]
pub struct ProjectListWidget {
    data: TimeTrackingData,
    project_list: ProjectList,
}

#[derive(Default, Debug)]
struct ProjectList {
    items: Vec<ProjectItem>,
    state: ListState,
}

#[derive(Debug)]
struct ProjectItem {
    name: String,
    total_hours: f32,
    tasks: Vec<String>,
}

impl ProjectListWidget {
    pub fn new(data: &TimeTrackingData) -> Self {
        let mut items: Vec<ProjectItem> = Vec::new();
        for project in &data.projects {
            let name = project.name.clone();
            let total_hours = project.total_minutes as f32 / 60.;
            let tasks = project.notes.clone();

            items.push(ProjectItem {
                name,
                total_hours,
                tasks,
            });
        }

        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }

        Self {
            data: data.clone(),
            project_list: ProjectList { items, state },
        }
    }

    /// Handle keyboard events for the project list
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        if self.project_list.items.is_empty() {
            return false;
        }

        match key_event.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_item();
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous_item();
                true
            }
            KeyCode::Char('g') => {
                self.go_to_first();
                true
            }
            KeyCode::Char('G') => {
                self.go_to_last();
                true
            }
            KeyCode::Enter => {
                self.copy_selected_notes_to_clipboard();
                true
            }
            // KeyCode::Left => {
            //     self.unselect();
            //     true
            // }
            _ => false,
        }
    }

    fn next_item(&mut self) {
        let i = match self.project_list.state.selected() {
            Some(i) => {
                if i >= self.project_list.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.project_list.state.select(Some(i));
    }

    fn previous_item(&mut self) {
        let i = match self.project_list.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.project_list.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.project_list.state.select(Some(i));
    }

    fn go_to_first(&mut self) {
        if !self.project_list.items.is_empty() {
            self.project_list.state.select(Some(0));
        }
    }

    fn go_to_last(&mut self) {
        if !self.project_list.items.is_empty() {
            self.project_list
                .state
                .select(Some(self.project_list.items.len() - 1));
        }
    }

    fn unselect(&mut self) {
        self.project_list.state.select(None);
    }

    fn copy_selected_notes_to_clipboard(&self) {
        if let Some(selected) = self.project_list.state.selected()
            && let Some(project) = self.project_list.items.get(selected)
        {
            // Placeholder for clipboard functionality
            // In a real implementation, you would use a crate like `copypasta` or `cli-clipboard`
            let notes_text = format!("- {}", project.tasks.join("\n- "));

            use copypasta::ClipboardProvider;
            let mut ctx = copypasta::ClipboardContext::new().unwrap();
            ctx.set_contents(notes_text).unwrap();
        }
    }

    pub fn selected_item(&self) -> Option<usize> {
        self.project_list.state.selected()
    }

    pub fn has_items(&self) -> bool {
        !self.project_list.items.is_empty()
    }
}

impl Widget for &mut ProjectListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(area);

        self.render_header(header_area, buf);
        self.render_footer(footer_area, buf);
        self.render_list(main_area, buf);
    }
}

impl ProjectListWidget {
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(format!(
            "Start Time: {}\n  End Time: {}\n\nWorking Time: {} hours",
            format_time_option(self.data.start_time.as_ref(), "N/A"),
            format_time_option(self.data.end_time.as_ref(), "N/A"),
            self.data.total_minutes as f32 / 60.
        ))
        .bold()
        .centered()
        .render(area, buf);
    }
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Use ↓↑ or j/k to move, g/G to go to the top or bottom, Enter to copy the notes for the current project to your clipboard.")
            .wrap(Wrap{trim: true})
            .centered()
            .render(area, buf);
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .title(Line::raw("Project Summaries").centered())
            .borders(Borders::TOP)
            .border_set(symbols::border::EMPTY)
            .border_style(TODO_HEADER_STYLE)
            .bg(NORMAL_ROW_BG);

        let items: Vec<ListItem> = self
            .project_list
            .items
            .iter()
            .enumerate()
            .map(|(i, project_item)| {
                let color = alternate_colors(i);
                ListItem::from(project_item).bg(color)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">>")
            .highlight_spacing(HighlightSpacing::Always)
            .repeat_highlight_symbol(true)
            .scroll_padding(1);

        StatefulWidget::render(list, area, buf, &mut self.project_list.state);
    }
}

const fn alternate_colors(i: usize) -> Color {
    if i.is_multiple_of(2) {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

impl From<&ProjectItem> for ListItem<'_> {
    fn from(value: &ProjectItem) -> Self {
        let mut text = String::new();
        if value.total_hours == 1. {
            text.push_str(&format!(" {:<25}{} hour\n", value.name, value.total_hours));
        } else {
            text.push_str(&format!(" {:<25}{} hours\n", value.name, value.total_hours));
        }

        for task in &value.tasks {
            text.push_str(&format!("   - {}\n", task));
        }
        ListItem::new(text)
    }
}
