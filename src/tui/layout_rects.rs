//! Where each clickable region was drawn on the most recent frame.
//!
//! Mouse hit-testing can only run after a draw, so this is filled in during
//! render and read by [`App::handle_mouse_event`](super::app::App). Every
//! field is cleared at the top of each frame: a region that was not drawn —
//! the calendar band on a short terminal, the week list while the day view
//! is up — must not be hittable, and `None` is what says so.

use ratatui::layout::Rect;

/// The regions the most recent frame drew, or `None` for regions it did not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutRects {
    /// The month calendar in the day view's header band.
    pub calendar: Option<Rect>,
    /// The weekly bar chart, in the band or full-screen when zoomed.
    pub bar_chart: Option<Rect>,
    /// The day view's project list, inside its border.
    pub project_list: Option<Rect>,
    /// The weekly rollup list, inside its border.
    pub week_list: Option<Rect>,
    /// The raw-file pane.
    pub raw_file: Option<Rect>,
    /// The footer, which carries the "press ? for help" hint.
    pub help_hint: Option<Rect>,
    /// The open overlay's box, if one is open. A click outside this while it
    /// is `Some` dismisses the overlay instead of reaching anything behind.
    pub overlay: Option<Rect>,
}

impl LayoutRects {
    /// Forget the previous frame. Called at the top of every render.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Does `rect` contain the cell at (`x`, `y`)?
///
/// `Rect::contains` exists in ratatui, but takes a `Position`; this keeps
/// the hit-test call sites reading in terminal coordinates.
pub fn hits(rect: Option<Rect>, x: u16, y: u16) -> bool {
    rect.is_some_and(|r| {
        x >= r.x && x < r.x.saturating_add(r.width) && y >= r.y && y < r.y.saturating_add(r.height)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_none_rect_is_never_hit() {
        assert!(!hits(None, 0, 0));
    }

    #[test]
    fn hits_are_inclusive_of_the_origin_and_exclusive_of_the_far_edge() {
        let r = Some(Rect::new(2, 3, 4, 5));
        assert!(hits(r, 2, 3), "origin");
        assert!(hits(r, 5, 7), "last cell");
        assert!(!hits(r, 6, 7), "one past the right edge");
        assert!(!hits(r, 5, 8), "one past the bottom edge");
        assert!(!hits(r, 1, 3), "one before the left edge");
    }

    #[test]
    fn clear_forgets_every_region() {
        let mut rects = LayoutRects {
            calendar: Some(Rect::new(0, 0, 1, 1)),
            ..LayoutRects::default()
        };
        rects.clear();
        assert_eq!(rects, LayoutRects::default());
    }
}
