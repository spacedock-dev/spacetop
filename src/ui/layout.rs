use ratatui::layout::Rect;

use crate::app::PickerState;

pub(super) enum PreviewPlacement {
    Left,
    Bottom,
}

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
