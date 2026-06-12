use ratatui::layout::Rect;

use crate::app::{PickerState, PreviewPlacement};

/// Geometric minimum pane width (columns) when splitting Left placement.
/// Together with the 10..=90 percent clamp in app-state, this keeps both
/// panes usable on small terminals. Pinned by tests.
const MIN_SPLIT_COLS: u16 = 10;

/// Geometric minimum pane height (rows) when splitting Bottom placement.
const MIN_SPLIT_ROWS: u16 = 3;

pub(super) fn picker_centered(area: Rect, state: &PickerState) -> Rect {
    const PICKER_WIDTH: u16 = 100;
    let width = area.width.min(PICKER_WIDTH);
    let extra = area.width.saturating_sub(width);
    let left = extra / 2;
    let workflow_rows = state.workflows().len().max(1) as u16;
    let chrome_rows = if state.error().is_some() { 7 } else { 6 };
    let height = area.height.min(workflow_rows + chrome_rows).max(8);
    let top = area.height.saturating_sub(height) / 2;
    Rect {
        x: area.x + left,
        y: area.y + top,
        width,
        height,
    }
}

pub(super) fn preview_placement(area: Rect) -> PreviewPlacement {
    if u32::from(area.width) > u32::from(area.height) * 2 {
        PreviewPlacement::Left
    } else {
        PreviewPlacement::Bottom
    }
}

/// Split the content area into (list, preview) rects, giving the list pane
/// `list_percent` of the split axis. Pure function — the single home for
/// the split geometry, so the divider-drag clamp behavior is testable
/// without a terminal backend. Both panes keep a minimum usable size
/// ([`MIN_SPLIT_COLS`] / [`MIN_SPLIT_ROWS`]) whenever the area allows it;
/// areas too small for both minimums fall back to a plain proportional
/// split.
pub(super) fn split_content(
    content: Rect,
    placement: PreviewPlacement,
    list_percent: u16,
) -> (Rect, Rect) {
    match placement {
        PreviewPlacement::Left => {
            let list_width = split_first_length(content.width, list_percent, MIN_SPLIT_COLS);
            let list = Rect {
                width: list_width,
                ..content
            };
            let preview = Rect {
                x: content.x + list_width,
                width: content.width - list_width,
                ..content
            };
            (list, preview)
        }
        PreviewPlacement::Bottom => {
            let list_height = split_first_length(content.height, list_percent, MIN_SPLIT_ROWS);
            let list = Rect {
                height: list_height,
                ..content
            };
            let preview = Rect {
                y: content.y + list_height,
                height: content.height - list_height,
                ..content
            };
            (list, preview)
        }
    }
}

/// Length of the first (list) pane when splitting `total` at `percent`,
/// clamped so both panes keep at least `min` cells when `total` allows it.
fn split_first_length(total: u16, percent: u16, min: u16) -> u16 {
    let proportional = (u32::from(total) * u32::from(percent.min(100)) / 100) as u16;
    if total >= min * 2 {
        proportional.clamp(min, total - min)
    } else {
        proportional.min(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(width: u16, height: u16) -> Rect {
        Rect {
            x: 2,
            y: 9,
            width,
            height,
        }
    }

    #[test]
    fn default_left_split_reproduces_historical_50_50() {
        let area = content(100, 20);
        let (list, preview) = split_content(area, PreviewPlacement::Left, 50);
        assert_eq!(
            list,
            Rect {
                x: 2,
                y: 9,
                width: 50,
                height: 20
            }
        );
        assert_eq!(
            preview,
            Rect {
                x: 52,
                y: 9,
                width: 50,
                height: 20
            }
        );
    }

    #[test]
    fn default_bottom_split_reproduces_historical_30_70() {
        let area = content(60, 30);
        let (list, preview) = split_content(area, PreviewPlacement::Bottom, 30);
        assert_eq!(
            list,
            Rect {
                x: 2,
                y: 9,
                width: 60,
                height: 9
            }
        );
        assert_eq!(
            preview,
            Rect {
                x: 2,
                y: 18,
                width: 60,
                height: 21
            }
        );
    }

    #[test]
    fn panes_tile_the_content_exactly() {
        for percent in [0, 10, 37, 50, 90, 100] {
            let area = content(81, 33);
            let (list, preview) = split_content(area, PreviewPlacement::Left, percent);
            assert_eq!(list.width + preview.width, area.width);
            assert_eq!(preview.x, list.x + list.width);
            let (list, preview) = split_content(area, PreviewPlacement::Bottom, percent);
            assert_eq!(list.height + preview.height, area.height);
            assert_eq!(preview.y, list.y + list.height);
        }
    }

    #[test]
    fn extreme_percents_clamp_to_minimum_pane_sizes() {
        let area = content(100, 20);
        let (list, preview) = split_content(area, PreviewPlacement::Left, 0);
        assert_eq!(list.width, MIN_SPLIT_COLS, "list keeps min cols at 0%");
        assert_eq!(preview.width, 90);
        let (list, preview) = split_content(area, PreviewPlacement::Left, 100);
        assert_eq!(
            preview.width, MIN_SPLIT_COLS,
            "preview keeps min cols at 100%"
        );
        assert_eq!(list.width, 90);

        let area = content(60, 30);
        let (list, _) = split_content(area, PreviewPlacement::Bottom, 0);
        assert_eq!(list.height, MIN_SPLIT_ROWS, "list keeps min rows at 0%");
        let (_, preview) = split_content(area, PreviewPlacement::Bottom, 100);
        assert_eq!(
            preview.height, MIN_SPLIT_ROWS,
            "preview keeps min rows at 100%"
        );
    }

    #[test]
    fn tiny_areas_fall_back_to_proportional_split_without_panic() {
        // Width below 2 * MIN_SPLIT_COLS: no room for both minimums.
        let area = content(15, 20);
        let (list, preview) = split_content(area, PreviewPlacement::Left, 50);
        assert_eq!(list.width + preview.width, 15);
        let (list, preview) = split_content(area, PreviewPlacement::Left, 100);
        assert_eq!((list.width, preview.width), (15, 0));
        // Degenerate zero-size area stays zero-size.
        let area = content(0, 0);
        let (list, preview) = split_content(area, PreviewPlacement::Bottom, 30);
        assert_eq!((list.height, preview.height), (0, 0));
    }
}
