//! Caret - Application state management

use crate::data::Dataset;
use crate::engine::{DedupEngine, DedupResult, DedupStrategy};
use crate::linter::LintResult;
use crate::tokenizer::{TiktokenEncoding, TokenizerWrapper};

/// View mode for the main display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Normal text view with syntax highlighting
    Text,
    /// Token X-Ray mode showing tokenization boundaries
    TokenXray,
    /// JSON tree view for nested structures
    Tree,
}

impl ViewMode {
    pub fn toggle(&mut self) {
        *self = match self {
            ViewMode::Text => ViewMode::TokenXray,
            ViewMode::TokenXray => ViewMode::Tree,
            ViewMode::Tree => ViewMode::Text,
        };
    }

    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::Text => "TEXT",
            ViewMode::TokenXray => "TOKEN X-RAY",
            ViewMode::Tree => "TREE",
        }
    }
}

/// Main application state
pub struct App {
    /// The loaded dataset
    pub dataset: Dataset,
    /// Current scroll position (line index)
    pub scroll: usize,
    /// Number of visible lines in the viewport
    pub viewport_height: usize,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Optional tokenizer for X-Ray mode
    pub tokenizer: Option<TokenizerWrapper>,
    /// Lint results for the current dataset
    pub lint_results: Vec<LintResult>,
    /// Deduplication scan results (None if no scan has been run)
    pub dedup_result: Option<DedupResult>,
    /// Optional JSON field path for field-specific dedup (dot-notation)
    pub dedup_field: Option<String>,
    /// Dedup strategy used by TUI's D key
    pub dedup_strategy: DedupStrategy,
    /// Whether to show the dedup group popup
    pub show_dedup_group: bool,
    /// Selected index within the dedup group popup list (left panel)
    pub dedup_group_selected: usize,
    /// Which panel has focus in the dedup group popup
    pub dedup_group_focus_right: bool,
    /// Scroll offset for the right panel in the dedup group popup
    pub dedup_group_detail_scroll: usize,
    /// Number of visible lines in the dedup group detail viewport (cached during render)
    pub dedup_group_detail_viewport_height: usize,
    /// Total number of lines in the dedup group detail content (cached during render)
    pub dedup_group_detail_content_lines: usize,
    /// Whether to show the help popup
    pub show_help: bool,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Currently selected line for details
    pub selected_line: usize,
    /// Whether to show the detail panel
    pub show_detail: bool,
    /// Scroll offset for the detail panel
    pub detail_scroll: usize,
    /// Number of visible lines in the detail panel viewport
    pub detail_viewport_height: usize,
    /// Total number of lines in the detail panel content (cached during render)
    pub detail_content_lines: usize,
    /// Whether the app is in search input mode
    pub search_mode: bool,
    /// Current search query string
    pub search_query: String,
    /// Line indices that match the search query
    pub search_matches: Vec<usize>,
    /// Current index into search_matches (0-based)
    pub search_current_idx: usize,
    /// Tree expansion state for JSON tree view
    #[allow(dead_code)]
    pub tree_expanded: std::collections::HashSet<String>,
    /// Currently selected token index in Token X-Ray mode (for hover details)
    pub selected_token: usize,
    /// Total token count for current line (cached for navigation bounds)
    pub token_count: usize,
}

impl App {
    /// Create a new app with the given dataset
    pub fn new(dataset: Dataset) -> Self {
        Self {
            dataset,
            scroll: 0,
            viewport_height: 20,
            view_mode: ViewMode::Text,
            tokenizer: None,
            lint_results: Vec::new(),
            dedup_result: None,
            dedup_field: None,
            dedup_strategy: DedupStrategy::Exact,
            show_dedup_group: false,
            dedup_group_selected: 0,
            dedup_group_focus_right: false,
            dedup_group_detail_scroll: 0,
            dedup_group_detail_viewport_height: 0,
            dedup_group_detail_content_lines: 0,
            show_help: false,
            should_quit: false,
            selected_line: 0,
            show_detail: false,
            detail_scroll: 0,
            detail_viewport_height: 0,
            detail_content_lines: 0,
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_current_idx: 0,
            tree_expanded: std::collections::HashSet::new(),
            selected_token: 0,
            token_count: 0,
        }
    }

    /// Toggle view mode, lazily loading the tokenizer when switching to Token X-Ray
    pub fn toggle_view_mode(&mut self) {
        // If switching from Text to TokenXray, lazily load tokenizer
        if self.view_mode == ViewMode::Text && self.tokenizer.is_none() {
            if let Ok(tokenizer) = TokenizerWrapper::from_tiktoken(TiktokenEncoding::Cl100kBase) {
                self.tokenizer = Some(tokenizer);
            }
        }
        self.view_mode.toggle();
    }

    /// Toggle detail panel visibility
    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
        self.detail_scroll = 0;
    }

    /// Toggle dedup scan: run if no result, clear if already scanned.
    /// Uses `dedup_strategy` and `dedup_field` if set.
    pub fn toggle_dedup(&mut self) {
        if self.dedup_result.is_some() {
            self.dedup_result = None;
            self.show_dedup_group = false;
        } else {
            let mut engine = DedupEngine::new(self.dedup_strategy);
            if let Some(ref field) = self.dedup_field {
                engine = engine.with_field(field.clone());
            }
            let result = engine.scan(&self.dataset);
            self.dedup_result = Some(result);
        }
    }

    /// Toggle the dedup group popup
    pub fn toggle_dedup_group(&mut self) {
        if self.dedup_result.is_some() {
            self.show_dedup_group = !self.show_dedup_group;
            self.dedup_group_selected = 0;
            self.dedup_group_focus_right = false;
            self.dedup_group_detail_scroll = 0;
        }
    }

    /// Move selection up in the dedup group popup (left panel)
    pub fn dedup_group_select_up(&mut self) {
        if self.dedup_group_selected > 0 {
            self.dedup_group_selected -= 1;
            self.dedup_group_detail_scroll = 0; // reset scroll when changing item
        }
    }

    /// Move selection down in the dedup group popup (left panel)
    pub fn dedup_group_select_down(&mut self) {
        if let Some(ref result) = self.dedup_result {
            let group = result.get_duplicate_group(self.selected_line);
            let max = group.len().saturating_sub(1);
            self.dedup_group_selected = self.dedup_group_selected.saturating_add(1).min(max);
            self.dedup_group_detail_scroll = 0; // reset scroll when changing item
        }
    }

    /// Scroll the dedup group detail panel down
    pub fn dedup_group_detail_scroll_down(&mut self, n: usize) {
        let max_scroll = self.dedup_group_detail_content_lines.saturating_sub(self.dedup_group_detail_viewport_height);
        self.dedup_group_detail_scroll = (self.dedup_group_detail_scroll + n).min(max_scroll);
    }

    /// Scroll the dedup group detail panel up
    pub fn dedup_group_detail_scroll_up(&mut self, n: usize) {
        self.dedup_group_detail_scroll = self.dedup_group_detail_scroll.saturating_sub(n);
    }

    /// Get the current line content
    pub fn current_line_content(&self) -> Option<&str> {
        self.dataset.get_line(self.selected_line)
    }

    /// Get pretty-printed JSON for current line
    pub fn current_line_pretty(&self) -> String {
        if let Some(line) = self.current_line_content() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| line.to_string())
            } else {
                line.to_string()
            }
        } else {
            String::new()
        }
    }

    /// Set the tokenizer for X-Ray mode
    pub fn with_tokenizer(mut self, tokenizer: TokenizerWrapper) -> Self {
        self.tokenizer = Some(tokenizer);
        self
    }

    /// Set lint results
    pub fn with_lint_results(mut self, results: Vec<LintResult>) -> Self {
        self.lint_results = results;
        self
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        let max_scroll = self
            .dataset
            .line_count()
            .saturating_sub(self.viewport_height);
        self.scroll = (self.scroll + n).min(max_scroll);
        self.selected_line =
            (self.selected_line + n).min(self.dataset.line_count().saturating_sub(1));
        self.selected_token = 0; // Reset token selection when changing lines
        self.detail_scroll = 0; // Reset detail scroll when changing lines
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
        self.selected_line = self.selected_line.saturating_sub(n);
        self.selected_token = 0; // Reset token selection when changing lines
        self.detail_scroll = 0; // Reset detail scroll when changing lines
    }

    /// Jump to the beginning
    pub fn goto_top(&mut self) {
        self.scroll = 0;
        self.selected_line = 0;
        self.selected_token = 0;
        self.detail_scroll = 0;
    }

    /// Jump to the end
    pub fn goto_bottom(&mut self) {
        let max_scroll = self
            .dataset
            .line_count()
            .saturating_sub(self.viewport_height);
        self.scroll = max_scroll;
        self.selected_line = self.dataset.line_count().saturating_sub(1);
        self.selected_token = 0;
        self.detail_scroll = 0;
    }

    /// Update viewport height based on terminal size
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height.saturating_sub(4); // Account for borders and status bar
    }

    /// Check if a line has lint errors
    pub fn line_has_error(&self, line_index: usize) -> bool {
        self.lint_results.iter().any(|r| r.line == line_index)
    }

    /// Check if a line is a duplicate (per dedup scan)
    pub fn line_is_duplicate(&self, line_index: usize) -> bool {
        self.dedup_result
            .as_ref()
            .map(|r| r.is_duplicate(line_index))
            .unwrap_or(false)
    }

    /// Get lint error for a specific line
    #[allow(dead_code)]
    pub fn get_lint_error(&self, line_index: usize) -> Option<&LintResult> {
        self.lint_results.iter().find(|r| r.line == line_index)
    }

    /// Navigate to the next token in Token X-Ray mode
    pub fn next_token(&mut self) {
        if self.token_count > 0 {
            self.selected_token = (self.selected_token + 1) % self.token_count;
        }
    }

    /// Navigate to the previous token in Token X-Ray mode
    pub fn prev_token(&mut self) {
        if self.token_count > 0 {
            self.selected_token = if self.selected_token == 0 {
                self.token_count.saturating_sub(1)
            } else {
                self.selected_token - 1
            };
        }
    }

    /// Update token count (called by UI when tokenizing current line)
    pub fn set_token_count(&mut self, count: usize) {
        self.token_count = count;
        if self.selected_token >= count {
            self.selected_token = 0;
        }
    }

    /// Scroll the detail panel down by n lines
    pub fn detail_scroll_down(&mut self, n: usize) {
        let max_scroll = self.detail_content_lines.saturating_sub(self.detail_viewport_height);
        self.detail_scroll = (self.detail_scroll + n).min(max_scroll);
    }

    /// Scroll the detail panel up by n lines
    pub fn detail_scroll_up(&mut self, n: usize) {
        self.detail_scroll = self.detail_scroll.saturating_sub(n);
    }

    // ─── Search ──────────────────────────────────────────────────────

    /// Enter search input mode
    pub fn enter_search(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_current_idx = 0;
    }

    /// Exit search mode (keep matches for n/N navigation)
    pub fn exit_search(&mut self) {
        self.search_mode = false;
    }

    /// Append a character to the search query
    pub fn search_push_char(&mut self, c: char) {
        self.search_query.push(c);
    }

    /// Delete the last character from the search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
    }

    /// Execute the search: find all lines matching the query (case-insensitive)
    pub fn execute_search(&mut self) {
        self.search_mode = false;
        if self.search_query.is_empty() {
            self.search_matches.clear();
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        self.search_matches = (0..self.dataset.line_count())
            .filter(|&i| {
                self.dataset.get_line(i)
                    .map(|l| l.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            })
            .collect();
        self.search_current_idx = 0;
        // Jump to first match
        self.jump_to_search_match();
    }

    /// Jump to the current search match, adjusting scroll
    fn jump_to_search_match(&mut self) {
        if let Some(&line_idx) = self.search_matches.get(self.search_current_idx) {
            self.selected_line = line_idx;
            // Center the match in the viewport
            if self.selected_line < self.viewport_height / 2 {
                self.scroll = 0;
            } else {
                self.scroll = self.selected_line - self.viewport_height / 2;
            }
        }
    }

    /// Jump to next search match
    pub fn next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_current_idx = (self.search_current_idx + 1) % self.search_matches.len();
        self.jump_to_search_match();
    }

    /// Jump to previous search match
    pub fn prev_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.search_current_idx == 0 {
            self.search_current_idx = self.search_matches.len() - 1;
        } else {
            self.search_current_idx -= 1;
        }
        self.jump_to_search_match();
    }

    /// Check if a line is a search match
    pub fn line_is_search_match(&self, line_index: usize) -> bool {
        self.search_matches.contains(&line_index)
    }

    /// Check if a line is the current search match
    pub fn line_is_current_search_match(&self, line_index: usize) -> bool {
        self.search_matches.get(self.search_current_idx) == Some(&line_index)
    }
}
