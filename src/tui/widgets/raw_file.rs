use std::path::Path;

use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::tui::theme::Theme;

/// Shown in place of the content when the active date has no file on disk
/// yet — distinct from an empty pane, which the missing raw text alone
/// cannot tell apart from a load still in flight.
const NO_FILE_TEXT: &str = "No file for this date yet.";

/// The active date's file exactly as it sits on disk.
///
/// The escape hatch for the prefix/suffix fencing feature: a day file can be
/// full of text yet parse to zero entries, and this is the only place that
/// text is shown, however it parses.
pub struct RawFileView<'a> {
    /// The file's path, shown as the pane's title.
    path: &'a Path,
    /// The file's content, or `None` when it does not exist on disk.
    content: Option<&'a str>,
    /// Lines scrolled past the top, as [`App::scroll_raw_file`] clamps it.
    ///
    /// [`App::scroll_raw_file`]: crate::tui::app::App::scroll_raw_file
    scroll: u16,
    theme: &'a Theme,
}

impl<'a> RawFileView<'a> {
    pub fn new(path: &'a Path, content: Option<&'a str>, scroll: u16, theme: &'a Theme) -> Self {
        Self {
            path,
            content,
            scroll,
            theme,
        }
    }

    /// The block every render draws: bordered, titled with the file's path.
    fn block(&self) -> Block<'static> {
        Block::bordered()
            .title(self.path.display().to_string())
            .border_type(BorderType::Rounded)
    }

    /// How many lines of content `area` could show, once the border this
    /// view draws is accounted for.
    ///
    /// A free-standing function of `area` alone — not a method on a built
    /// `RawFileView` — so [`crate::tui::ui`]'s render pass can record it for
    /// [`App::scroll_raw_file`]'s upper clamp without first assembling the
    /// content this view would draw. Computed the same way [`Self::render`]
    /// lays the pane out, so the two can never drift apart.
    ///
    /// [`App::scroll_raw_file`]: crate::tui::app::App::scroll_raw_file
    pub fn visible_lines(area: Rect) -> u16 {
        Block::bordered().inner(area).height
    }
}

impl Widget for RawFileView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = self.block();
        let inner = block.inner(area);
        block.render(area, buf);

        match self.content {
            Some(content) => Paragraph::new(content)
                .scroll((self.scroll, 0))
                .render(inner, buf),
            None => Paragraph::new(NO_FILE_TEXT)
                .style(self.theme.warning)
                .alignment(Alignment::Center)
                .render(inner, buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn rendered(view: RawFileView<'_>, w: u16, h: u16) -> String {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        view.render(buf.area, &mut buf);
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_title_names_the_files_path() {
        let theme = Theme::none();
        let path = PathBuf::from("/tmp/2026-08-27.md");
        let screen = rendered(
            RawFileView::new(&path, Some("8-10 admin"), 0, &theme),
            40,
            10,
        );
        assert!(screen.contains("2026-08-27.md"), "got:\n{screen}");
    }

    #[test]
    fn visible_lines_matches_what_render_actually_draws() {
        let area = Rect::new(0, 0, 40, 10);
        // Content taller than the pane, so every visible row is spoken for
        // and the count can be read straight off the screen.
        let content = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let theme = Theme::none();
        let path = PathBuf::from("/tmp/2026-08-27.md");
        let screen = rendered(RawFileView::new(&path, Some(&content), 0, &theme), 40, 10);

        // Each row also carries the block's left border character, so a
        // content row reads e.g. "│line 3" rather than starting with it.
        let shown = screen.lines().filter(|line| line.contains("line ")).count();
        assert_eq!(shown as u16, RawFileView::visible_lines(area));
    }
}
