use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Layout, Rect},
    style::{
        Color, Modifier, Style, Stylize,
        palette::tailwind::{BLUE, GREEN, SLATE},
    },
    symbols,
    text::Line,
    widgets::{
        Block, Borders, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph,
        StatefulWidget, Widget, Wrap,
    },
};
use time_tracking_parser::TimeTrackingData;
const TODO_HEADER_STYLE: Style = Style::new().fg(SLATE.c100).bg(BLUE.c800);
const NORMAL_ROW_BG: Color = SLATE.c950;
const ALT_ROW_BG_COLOR: Color = SLATE.c900;
const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);
const TEXT_FG_COLOR: Color = SLATE.c200;
const COMPLETED_TEXT_FG_COLOR: Color = GREEN.c500;

#[derive(Default)]
pub struct ProjectListWidget {
    data: TimeTrackingData,
    project_list: ProjectList,
}

#[derive(Default)]
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

        Self {
            data: data.clone(),
            project_list: ProjectList {
                items,
                state: ListState::default(),
            },
        }
    }
}

impl Widget for &mut ProjectListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
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
            "Start Time: {:?}\nEnd Time: {:?}\n\nWorking Time: {} hours",
            self.data.start_time,
            self.data.end_time,
            self.data.total_minutes as f32 / 60.
        ))
        .bold()
        .centered()
        .render(area, buf);
    }
    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Use ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.")
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
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.project_list.state);
    }
}

const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        NORMAL_ROW_BG
    } else {
        ALT_ROW_BG_COLOR
    }
}

impl From<&ProjectItem> for ListItem<'_> {
    fn from(value: &ProjectItem) -> Self {
        let mut text = String::new();
        text.push_str(&format!(
            "Project: {}\tTotal Hours: {}\n",
            value.name, value.total_hours
        ));
        for task in &value.tasks {
            text.push_str(&format!("{}\n", task));
        }
        ListItem::new(text)
    }
}
