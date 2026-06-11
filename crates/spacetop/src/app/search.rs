#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Search,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    Metrics,
    Activity,
    Timeline,
    Relations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEntry {
    pub label: &'static str,
    pub action: CommandAction,
}

pub const SEARCH_VISIBLE_RESULT_LIMIT: usize = 8;

pub const COMMAND_ENTRIES: &[CommandEntry] = &[
    CommandEntry {
        label: "metrics",
        action: CommandAction::Metrics,
    },
    CommandEntry {
        label: "activity",
        action: CommandAction::Activity,
    },
    CommandEntry {
        label: "timeline",
        action: CommandAction::Timeline,
    },
    CommandEntry {
        label: "relations",
        action: CommandAction::Relations,
    },
];

pub fn matching_commands(query: &str) -> Vec<CommandEntry> {
    let needle = query.to_lowercase();
    COMMAND_ENTRIES
        .iter()
        .copied()
        .filter(|entry| needle.is_empty() || entry.label.starts_with(&needle))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchState {
    mode: SearchMode,
    query: String,
    selected_index: usize,
}

impl SearchState {
    pub fn new(mode: SearchMode) -> Self {
        Self {
            mode,
            query: String::new(),
            selected_index: 0,
        }
    }

    pub fn mode(&self) -> SearchMode {
        self.mode
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn push(&mut self, ch: char) {
        if !ch.is_control() {
            self.query.push(ch);
            self.selected_index = 0;
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected_index = 0;
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn select_next(&mut self, len: usize) {
        if len > 0 {
            self.selected_index = (self.selected_index + 1).min(len - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_backspace_is_safe_on_empty_query() {
        let mut state = SearchState::new(SearchMode::Search);
        state.backspace();
        assert_eq!(state.query(), "");
    }

    #[test]
    fn command_palette_starts_with_empty_query() {
        let state = SearchState::new(SearchMode::Command);
        assert_eq!(state.mode(), SearchMode::Command);
        assert_eq!(state.query(), "");
    }

    #[test]
    fn typed_input_resets_selection_and_backspace_updates_query() {
        let mut state = SearchState::new(SearchMode::Command);
        state.push('m');
        state.select_next(2);
        state.push('e');
        assert_eq!(state.query(), "me");
        assert_eq!(state.selected_index(), 0);
        state.backspace();
        assert_eq!(state.query(), "m");
    }

    #[test]
    fn selection_moves_within_result_bounds() {
        let mut state = SearchState::new(SearchMode::Command);
        state.select_next(2);
        state.select_next(2);
        assert_eq!(state.selected_index(), 1);
        state.select_previous();
        assert_eq!(state.selected_index(), 0);
    }
}
