//! Caret - UI rendering
//!
//! Renders the main interface using Ratatui widgets.

use crate::app::{App, ViewMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

/// Theme colors for the UI
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub error: Color,
    pub warning: Color,
    pub border: Color,
    pub highlight: Color,
    pub muted: Color,
    pub duplicate: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Dracula-inspired dark theme
        Self {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(139, 233, 253),
            error: Color::Rgb(255, 85, 85),
            warning: Color::Rgb(255, 184, 108),
            border: Color::Rgb(98, 114, 164),
            highlight: Color::Rgb(68, 71, 90),
            muted: Color::Rgb(98, 114, 164),
            duplicate: Color::Rgb(255, 170, 50), // Amber for duplicates
        }
    }
}

/// Render the entire UI
pub fn render(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();

    // Main layout: content area + (search bar if active) + status bar
    let main_chunks = if app.search_mode {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3), Constraint::Length(3)])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(frame.area())
    };

    let content_area = main_chunks[0];
    let status_area = *main_chunks.last().unwrap();

    // Update viewport height based on content area size
    app.set_viewport_height(content_area.height as usize);

    // If detail panel is visible, split content area horizontally
    if app.show_detail {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_area);

        render_content(frame, app, content_chunks[0], &theme);
        render_detail_panel(frame, app, content_chunks[1], &theme);
    } else {
        render_content(frame, app, content_area, &theme);
    }

    // Render search bar if in search mode
    if app.search_mode {
        render_search_bar(frame, app, main_chunks[1], &theme);
    }

    // Render status bar
    render_status_bar(frame, app, status_area, &theme);

    // Render help popup if visible
    if app.show_help {
        render_help_popup(frame, &theme);
    }

    // Render dedup group popup if visible
    if app.show_dedup_group {
        render_dedup_group_popup(frame, app, &theme);
    }
}

/// Render the main content area with dataset lines
fn render_content(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let visible_lines = (area.height as usize).saturating_sub(2);

    let items: Vec<ListItem> = (0..visible_lines)
        .filter_map(|i| {
            let line_idx = app.scroll + i;
            let line_content = app.dataset.get_line(line_idx)?;

            // Truncate long lines for display (respecting UTF-8 char boundaries)
            let display_width = area.width as usize - 10;
            let truncated = if line_content.len() > display_width {
                let mut end = display_width.saturating_sub(3);
                while !line_content.is_char_boundary(end) && end > 0 {
                    end -= 1;
                }
                format!("{}...", &line_content[..end])
            } else {
                line_content.to_string()
            };

            // Create styled line based on view mode, lint status, and dedup status
            let line: Line = if app.line_has_error(line_idx) {
                // Error line - highlight in red (highest priority)
                Line::from(vec![
                    Span::styled(
                        format!("{:>6} │ ", line_idx + 1),
                        Style::default().fg(theme.error),
                    ),
                    Span::styled(
                        truncated,
                        Style::default()
                            .fg(theme.error)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            } else if app.line_is_duplicate(line_idx) {
                // Duplicate line - highlight in amber, show canonical line number
                let canonical = app.dedup_result.as_ref()
                    .map(|r| r.canonical_line(line_idx))
                    .unwrap_or(line_idx);
                Line::from(vec![
                    Span::styled(
                        format!("{:>6} │ ", line_idx + 1),
                        Style::default().fg(theme.duplicate),
                    ),
                    Span::styled(
                        format!("DUP→{} ", canonical + 1),
                        Style::default()
                            .fg(Color::Rgb(40, 42, 54))
                            .bg(theme.duplicate)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        truncated,
                        Style::default().fg(theme.duplicate),
                    ),
                ])
            } else if app.dedup_result.as_ref().is_some_and(|r| {
                // Canonical (origin) line that has duplicates pointing to it
                !r.is_duplicate(line_idx) && r.canonical_group_size(line_idx) > 1
            }) {
                let dup_count = app.dedup_result.as_ref()
                    .map(|r| r.canonical_group_size(line_idx) - 1)
                    .unwrap_or(0);
                Line::from(vec![
                    Span::styled(
                        format!("{:>6} │ ", line_idx + 1),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        format!("ORIG×{} ", dup_count),
                        Style::default()
                            .fg(Color::Rgb(40, 42, 54))
                            .bg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(truncated, Style::default().fg(theme.fg)),
                ])
            } else if app.view_mode == ViewMode::TokenXray {
                // Token X-Ray mode
                if let Some(ref tokenizer) = app.tokenizer {
                    let token_line = tokenizer.colorize_tokens(&truncated);
                    let mut spans = vec![Span::styled(
                        format!("{:>6} │ ", line_idx + 1),
                        Style::default().fg(theme.muted),
                    )];
                    spans.extend(token_line.spans);
                    Line::from(spans)
                } else {
                    Line::from(vec![
                        Span::styled(
                            format!("{:>6} │ ", line_idx + 1),
                            Style::default().fg(theme.muted),
                        ),
                        Span::styled(truncated, Style::default().fg(theme.fg)),
                    ])
                }
            } else {
                // Normal text mode with JSON syntax highlighting
                let highlighted = highlight_json(&truncated, theme);
                let mut spans = vec![Span::styled(
                    format!("{:>6} │ ", line_idx + 1),
                    Style::default().fg(theme.muted),
                )];
                spans.extend(highlighted.spans);
                Line::from(spans)
            };

            // Highlight selected line
            let style = if line_idx == app.selected_line {
                Style::default().bg(theme.highlight)
            } else if app.line_is_current_search_match(line_idx) {
                Style::default().bg(Color::Rgb(80, 60, 30)) // Warm highlight for current match
            } else if app.line_is_search_match(line_idx) {
                Style::default().bg(Color::Rgb(55, 55, 40)) // Dim highlight for other matches
            } else {
                Style::default()
            };

            Some(ListItem::new(line).style(style))
        })
        .collect();

    let mode_indicator = format!(" {} ", app.view_mode.label());
    let dedup_indicator = if app.dedup_result.is_some() {
        " | DEDUP"
    } else {
        ""
    };
    let title = format!(
        " Caret │ {} │ {} lines │ {}{}  ",
        app.dataset.path.split('/').next_back().unwrap_or("file"),
        app.dataset.line_count(),
        mode_indicator,
        dedup_indicator,
    );

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(Span::styled(title, Style::default().fg(theme.accent)))
            .style(Style::default().bg(theme.bg)),
    );

    frame.render_widget(list, area);
}

/// Render the search input bar
fn render_search_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let cursor = "█";
    let search_line = Line::from(vec![
        Span::styled(
            " /",
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}{}", app.search_query, cursor),
            Style::default().fg(theme.fg),
        ),
    ]);

    let search_bar = Paragraph::new(search_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .style(Style::default().bg(theme.bg)),
        );

    frame.render_widget(search_bar, area);
}

/// Render the status bar
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let lint_count = app.lint_results.len();
    let lint_status = if lint_count > 0 {
        format!(" {} issues ", lint_count)
    } else {
        " No issues ".to_string()
    };

    let lint_style = if lint_count > 0 {
        Style::default().fg(theme.warning)
    } else {
        Style::default().fg(Color::Rgb(80, 250, 123)) // Green
    };

    let tokenizer_status = if let Some(ref t) = app.tokenizer {
        format!(" {} ", t.name)
    } else {
        " No tokenizer ".to_string()
    };

    let dedup_status = if let Some(ref result) = app.dedup_result {
        let field_info = app.dedup_field.as_deref().unwrap_or("all");
        format!(
            " {} dups ({:.0}%) {:.0}ms [{}] ",
            result.duplicate_count,
            result.dedup_ratio() * 100.0,
            result.elapsed_us as f64 / 1000.0,
            field_info,
        )
    } else if app.dedup_field.is_some() {
        format!(" dedup-field:{} ", app.dedup_field.as_deref().unwrap())
    } else {
        String::new()
    };

    let dedup_style = Style::default().fg(theme.duplicate);

    let position = format!(
        " Line {}/{} ",
        app.selected_line + 1,
        app.dataset.line_count()
    );

    let mut spans = vec![
        Span::styled(
            concat!(" Caret v", env!("CARGO_PKG_VERSION"), " "),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("|", Style::default().fg(theme.border)),
        Span::styled(
            format!(" {} ", app.dataset.size_human()),
            Style::default().fg(theme.fg),
        ),
        Span::styled("|", Style::default().fg(theme.border)),
        Span::styled(lint_status, lint_style),
        Span::styled("|", Style::default().fg(theme.border)),
        Span::styled(tokenizer_status, Style::default().fg(theme.muted)),
    ];

    if !dedup_status.is_empty() {
        spans.push(Span::styled("|", Style::default().fg(theme.border)));
        spans.push(Span::styled(dedup_status, dedup_style));
    }

    if !app.search_matches.is_empty() {
        let search_status = format!(
            " /{} {}/{} ",
            app.search_query,
            app.search_current_idx + 1,
            app.search_matches.len(),
        );
        spans.push(Span::styled("|", Style::default().fg(theme.border)));
        spans.push(Span::styled(search_status, Style::default().fg(theme.accent)));
    }

    spans.push(Span::styled("|", Style::default().fg(theme.border)));
    spans.push(Span::styled(position, Style::default().fg(theme.fg)));
    spans.push(Span::styled("|", Style::default().fg(theme.border)));
    spans.push(Span::styled(" ?:Help q:Quit ", Style::default().fg(theme.muted)));

    let status_line = Line::from(spans);

    let status_bar = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg)),
    );

    frame.render_widget(status_bar, area);
}

/// Render help popup
fn render_help_popup(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(55, 80, frame.area());

    // Clear the background
    frame.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  ↑ / ↓    ", Style::default().fg(theme.warning)),
            Span::raw("Move between lines"),
        ]),
        Line::from(vec![
            Span::styled("  d / f    ", Style::default().fg(theme.warning)),
            Span::raw("Scroll detail / move (d=up f=down)"),
        ]),
        Line::from(vec![
            Span::styled("  g        ", Style::default().fg(theme.warning)),
            Span::raw("Go to top"),
        ]),
        Line::from(vec![
            Span::styled("  G        ", Style::default().fg(theme.warning)),
            Span::raw("Go to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d   ", Style::default().fg(theme.warning)),
            Span::raw("Page down"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+u   ", Style::default().fg(theme.warning)),
            Span::raw("Page up"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Search",
            Style::default()
                .fg(Color::Rgb(255, 184, 108))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(Color::Rgb(255, 184, 108))),
            Span::raw("Enter search mode"),
        ]),
        Line::from(vec![
            Span::styled("  n / N    ", Style::default().fg(Color::Rgb(255, 184, 108))),
            Span::raw("Next / previous match"),
        ]),
        Line::from(vec![
            Span::styled("  Esc      ", Style::default().fg(Color::Rgb(255, 184, 108))),
            Span::raw("Cancel search input"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "View Modes",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Tab      ", Style::default().fg(theme.accent)),
            Span::raw("Cycle: TEXT -> TOKEN X-RAY -> TREE"),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(theme.accent)),
            Span::raw("Toggle detail panel (pretty JSON)"),
        ]),
        Line::from(vec![
            Span::styled("  d / f    ", Style::default().fg(theme.accent)),
            Span::raw("Scroll detail panel"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+d/u ", Style::default().fg(theme.accent)),
            Span::raw("Half-page scroll (detail if open)"),
        ]),
        Line::from(vec![
            Span::styled("  PgDn/Up  ", Style::default().fg(theme.accent)),
            Span::raw("Full-page scroll (detail if open)"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Analysis",
            Style::default()
                .fg(theme.duplicate)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  D        ", Style::default().fg(theme.duplicate)),
            Span::raw("Toggle dedup scan (SimHash)"),
        ]),
        Line::from(vec![
            Span::styled("  O        ", Style::default().fg(theme.duplicate)),
            Span::raw("Duplicate group popup (←/→:switch d/f:scroll)"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ?        ", Style::default().fg(theme.muted)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q        ", Style::default().fg(theme.error)),
            Span::raw("Quit"),
        ]),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(Span::styled(" Help ", Style::default().fg(theme.accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.bg)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

/// Calculate the total number of visual rows after word-wrapping,
/// using the same textwrap algorithm that ratatui uses internally.
fn wrapped_line_count(lines: &[Line<'_>], available_width: u16) -> usize {
    if available_width == 0 {
        return lines.len();
    }
    let options = textwrap::Options::new(available_width as usize);
    let mut total = 0;
    for line in lines {
        // Extract plain text from all spans
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if text.is_empty() {
            total += 1; // empty line still takes one row
        } else {
            total += textwrap::wrap(&text, &options).len();
        }
    }
    total
}

/// Render dedup group popup: left panel = group list, right panel = detail view
fn render_dedup_group_popup(frame: &mut Frame, app: &mut App, theme: &Theme) {
    let Some(ref dedup_result) = app.dedup_result else {
        return;
    };

    let group = dedup_result.get_duplicate_group(app.selected_line);
    if group.len() <= 1 {
        return; // Not part of a duplicate group
    }

    // Clamp selection to valid range
    app.dedup_group_selected = app.dedup_group_selected.min(group.len() - 1);
    let selected = app.dedup_group_selected;

    let canonical = dedup_result.canonical_line(app.selected_line);

    // Use full-width popup, split into left (list) and right (detail)
    let area = centered_rect(90, 80, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let left_focused = !app.dedup_group_focus_right;
    let left_border = if left_focused { theme.accent } else { theme.border };

    // === Left panel: group list ===
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        format!("Group (canonical: L{})", canonical + 1),
        Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, &line_idx) in group.iter().enumerate() {
        let is_canonical = line_idx == canonical;
        let is_selected = i == selected;

        let tag = if is_canonical { "ORIG" } else { "DUP" };
        let tag_color = if is_canonical { theme.accent } else { theme.duplicate };
        let marker = if is_selected { "▸" } else { " " };

        let preview = app.dataset.get_line(line_idx)
            .map(|l| {
                let s: String = l.chars().take(50).collect();
                if l.len() > 50 { format!("{}...", s) } else { s }
            })
            .unwrap_or_default();

        let bg = if is_selected && left_focused { theme.highlight } else { theme.bg };
        let fg = if is_selected { theme.fg } else { theme.muted };

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", marker), Style::default().fg(theme.fg)),
            Span::styled(
                format!("L{:>5} ", line_idx + 1),
                Style::default().fg(tag_color),
            ),
            Span::styled(
                format!("[{}] ", tag),
                Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(preview, Style::default().fg(fg).bg(bg)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "←/→:switch panel",
        Style::default().fg(theme.muted),
    )));

    let left_panel = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(" Dedup Group ", Style::default().fg(theme.accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(left_border))
                .style(Style::default().bg(theme.bg)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(left_panel, chunks[0]);

    // === Right panel: detail of selected item ===
    let right_focused = app.dedup_group_focus_right;
    let right_border = if right_focused { theme.accent } else { theme.border };

    let &line_idx = group.get(selected).unwrap_or(&app.selected_line);
    let is_canonical = line_idx == canonical;
    let tag = if is_canonical { "ORIG" } else { "DUP" };
    let tag_color = if is_canonical { theme.accent } else { theme.duplicate };

    let mut detail = Vec::new();
    detail.push(Line::from(vec![
        Span::styled(
            format!("Line {} ", line_idx + 1),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[{}]", tag),
            Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
        ),
    ]));
    detail.push(Line::from(""));

    // Pretty-print the JSON content
    if let Some(line) = app.dataset.get_line(line_idx) {
        let pretty = if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| line.to_string())
        } else {
            line.to_string()
        };
        for l in pretty.lines() {
            detail.push(highlight_json(l, theme));
        }
    }

    // Update scroll bounds for right panel
    let inner_width = chunks[1].width.saturating_sub(2);
    app.dedup_group_detail_content_lines = wrapped_line_count(&detail, inner_width);
    app.dedup_group_detail_viewport_height = chunks[1].height.saturating_sub(2) as usize;

    let right_panel = Paragraph::new(detail)
        .block(
            Block::default()
                .title(Span::styled(" Detail ", Style::default().fg(theme.accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(right_border))
                .style(Style::default().bg(theme.bg)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.dedup_group_detail_scroll as u16, 0));

    frame.render_widget(right_panel, chunks[1]);
}

/// Render the detail panel showing pretty-printed JSON
fn render_detail_panel(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    // In Token X-Ray mode with tokenizer, show hover-style token details
    if app.view_mode == ViewMode::TokenXray && app.tokenizer.is_some() {
        render_token_xray_hover(frame, app, area, theme);
        return;
    }

    let pretty_json = app.current_line_pretty();

    // Default: show pretty JSON with syntax highlighting
    let lines: Vec<Line> = pretty_json
        .lines()
        .map(|line| highlight_json(line, theme))
        .collect();

    app.detail_content_lines = wrapped_line_count(&lines, area.width.saturating_sub(2));
    app.detail_viewport_height = area.height.saturating_sub(2) as usize; // subtract borders

    let dup_label = if app.line_is_duplicate(app.selected_line) {
        " [DUPLICATE]"
    } else {
        ""
    };

    let title = format!(" Record {}{} ", app.selected_line + 1, dup_label);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(title, Style::default().fg(theme.accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.bg)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

/// Render token X-Ray with hover-style details (selected token info at bottom)
fn render_token_xray_hover(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
) {
    use crate::tokenizer::TokenInfo;

    // Color palette for tokens
    const TOKEN_COLORS: [Color; 4] = [
        Color::Rgb(70, 130, 180),  // Steel Blue
        Color::Rgb(60, 60, 60),    // Dark Gray
        Color::Rgb(100, 149, 237), // Cornflower Blue
        Color::Rgb(80, 80, 80),    // Medium Gray
    ];
    const HIGHLIGHT_COLOR: Color = Color::Rgb(255, 200, 50); // Gold for selected

    // Collect all data we need from app first (before mutation)
    let (all_tokens, pretty_json, line_tokenizations): (Vec<TokenInfo>, String, Vec<Vec<TokenInfo>>) = {
        let tokenizer = app.tokenizer.as_ref().unwrap();
        let pretty_json = app.current_line_pretty();
        let raw_line = app.current_line_content().unwrap_or("").to_string();
        
        // Get all tokens from raw line for the status bar
        let all_tokens = tokenizer.get_token_details(&raw_line);
        
        // Pre-tokenize each JSON line for display
        let line_tokenizations: Vec<Vec<TokenInfo>> = pretty_json
            .lines()
            .map(|line| tokenizer.get_token_details(line))
            .collect();
        
        (all_tokens, pretty_json, line_tokenizations)
    };
    
    // Now we can mutate app safely
    app.set_token_count(all_tokens.len());

    // Get the selected token info (if any)
    let selected_info: Option<&TokenInfo> = all_tokens.get(app.selected_token);

    // Split area: main content + status line at bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    // === Render main content area with colored tokens ===
    let mut lines = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(
            "TOKEN X-RAY: ".to_string(),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("←/→ navigate ({}/{})", app.selected_token + 1, all_tokens.len().max(1)),
            Style::default().fg(theme.muted),
        ),
    ]));
    lines.push(Line::from(""));

    // Show each JSON line with tokenization
    for (json_line, line_tokens) in pretty_json.lines().zip(line_tokenizations.iter()) {
        if line_tokens.is_empty() {
            lines.push(highlight_json(json_line, theme));
            continue;
        }

        let mut spans = Vec::new();
        for (i, token) in line_tokens.iter().enumerate() {
            // Check if this token matches the selected one (by position in raw line)
            let is_selected = selected_info.is_some_and(|sel| {
                token.byte_start == sel.byte_start && token.byte_end == sel.byte_end
            });

            let bg_color = if is_selected {
                HIGHLIGHT_COLOR
            } else {
                TOKEN_COLORS[i % TOKEN_COLORS.len()]
            };

            let fg_color = if is_selected {
                Color::Black
            } else {
                Color::White
            };

            spans.push(Span::styled(
                token.text.clone(),
                Style::default().bg(bg_color).fg(fg_color),
            ));
        }
        lines.push(Line::from(spans));
    }

    // Update detail panel content line count and viewport height
    app.detail_content_lines = wrapped_line_count(&lines, chunks[0].width.saturating_sub(2));
    app.detail_viewport_height = chunks[0].height.saturating_sub(2) as usize; // subtract borders

    let dup_label = if app.line_is_duplicate(app.selected_line) {
        " [DUP]"
    } else {
        ""
    };

    let main_content = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" Record {}{} ", app.selected_line + 1, dup_label),
                    Style::default().fg(theme.accent),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.bg)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll as u16, 0));

    frame.render_widget(main_content, chunks[0]);

    // === Render token detail status line at bottom ===
    let detail_line = if let Some(tok) = selected_info {
        Line::from(vec![
            Span::styled(" Token: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("\"{}\"", tok.text.replace('\n', "\\n").replace('\t', "\\t")),
                Style::default().fg(HIGHLIGHT_COLOR).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ID: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}", tok.token_id),
                Style::default().fg(theme.accent),
            ),
            Span::styled(" │ Bytes: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}-{}", tok.byte_start, tok.byte_end),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!(" ({} bytes)", tok.byte_end - tok.byte_start),
                Style::default().fg(theme.muted),
            ),
        ])
    } else {
        Line::from(Span::styled(
            " No tokens",
            Style::default().fg(theme.muted),
        ))
    };

    let status_bar = Paragraph::new(detail_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .style(Style::default().bg(theme.bg)),
        );

    frame.render_widget(status_bar, chunks[1]);
}

/// Basic JSON syntax highlighting
fn highlight_json(text: &str, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();
    let mut is_key = true;

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }

                // Find the end of the string
                let mut string_content = String::from('"');
                let mut escaped = false;
                for ch in chars.by_ref() {
                    string_content.push(ch);
                    if ch == '"' && !escaped {
                        break;
                    }
                    escaped = ch == '\\' && !escaped;
                }

                let color = if is_key {
                    theme.accent
                } else {
                    Color::Rgb(241, 250, 140) // Yellow for values
                };
                spans.push(Span::styled(string_content, Style::default().fg(color)));
            }
            ':' => {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                spans.push(Span::styled(":", Style::default().fg(theme.fg)));
                is_key = false;
            }
            ',' => {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                spans.push(Span::styled(",", Style::default().fg(theme.fg)));
                is_key = true;
            }
            '{' | '}' | '[' | ']' => {
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(theme.warning),
                ));
                if c == '{' || c == '[' {
                    is_key = c == '{';
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        // Check if it's a number or boolean
        let trimmed = current.trim();
        let color = if trimmed.parse::<f64>().is_ok() {
            Color::Rgb(189, 147, 249) // Purple for numbers
        } else if trimmed == "true" || trimmed == "false" || trimmed == "null" {
            Color::Rgb(255, 121, 198) // Pink for booleans
        } else {
            theme.fg
        };
        spans.push(Span::styled(current, Style::default().fg(color)));
    }

    Line::from(spans)
}

/// Helper to create a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
