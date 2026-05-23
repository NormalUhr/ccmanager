use crate::config::KeyBindings;
use crate::tui::app::{
    App, AppMode, DialogMode, LineStyle, RenderedLine, ViewSearchMode, ViewState,
};
use crate::tui::theme::{self, Theme};
use chrono::{DateTime, Local};
use ratatui::layout::Position;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use unicode_width::UnicodeWidthChar;

/// Get the current theme
fn th() -> &'static Theme {
    theme::detect_theme()
}

/// Convert theme RGB tuple to ratatui Color
fn rgb(c: (u8, u8, u8)) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// State of the header band — drives the right-hand metadata.
pub enum HeaderState<'a> {
    Idle {
        scope: &'a str,
        total: usize,
    },
    Search {
        scope: &'a str,
        matched: usize,
        total: usize,
    },
    Loading {
        scope: &'a str,
        so_far: usize,
    },
}

/// Render the header band as a single styled line:
///   `◈ ccmanager  ·  <scope>  ·  <count metadata>`
pub fn header_line(theme: &Theme, state: &HeaderState<'_>) -> Line<'static> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;

    let accent = Style::default()
        .fg(rgb(theme.accent))
        .add_modifier(Modifier::BOLD);
    let primary = Style::default().fg(rgb(theme.text_primary));
    let dim = Style::default().fg(rgb(theme.text_muted));
    let sep = || Span::styled("  ·  ", dim);

    let (scope_text, right): (&str, String) = match state {
        HeaderState::Idle { scope, total } => (scope, format!("{} sessions", total)),
        HeaderState::Search {
            scope,
            matched,
            total,
        } => (scope, format!("{} / {} sessions match", matched, total)),
        HeaderState::Loading { scope, so_far } => (scope, format!("loading… {} so far", so_far)),
    };

    Line::from(vec![
        Span::styled("  ◈ ccmanager", accent),
        sep(),
        Span::styled(scope_text.to_string(), primary),
        sep(),
        Span::styled(right, dim),
    ])
}

/// State passed to `footer_line` — controls which key hints are shown.
pub enum FooterState<'a> {
    ListIdle,
    #[allow(dead_code)]
    Viewer,
    #[allow(dead_code)]
    ViewerMessageNav,
    /// Viewer with in-conversation search active. Shows vim-style nav.
    ViewerSearchActive {
        current: usize,
        total: usize,
    },
    StatusMessage(&'a str),
}

/// Render the footer band as a single styled line.
///
/// Keys are rendered in `text_primary`, their one-word descriptions in
/// `text_muted`; status messages take over the whole line in `accent`.
/// When `compact` is true and the state is `ListIdle`, a shortened set
/// of hints is shown that fits narrow/short terminals.
pub fn footer_line(theme: &Theme, state: &FooterState<'_>, compact: bool) -> Line<'static> {
    use ratatui::style::Style;
    use ratatui::text::Span;

    let key = Style::default().fg(rgb(theme.text_primary));
    let desc = Style::default().fg(rgb(theme.text_muted));
    let accent = Style::default().fg(rgb(theme.accent));

    match state {
        FooterState::StatusMessage(msg) => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled(msg.to_string(), accent),
        ]),
        FooterState::ListIdle => {
            if compact {
                return Line::from(vec![
                    Span::styled("  ", desc),
                    Span::styled("↑↓", key),
                    Span::styled(" ", desc),
                    Span::styled("/", key),
                    Span::styled(" search ", desc),
                    Span::styled("⏎", key),
                    Span::styled(" view ", desc),
                    Span::styled("?", key),
                    Span::styled(" help", desc),
                ]);
            }
            Line::from(vec![
                Span::styled("  ", desc),
                Span::styled("↑↓", key),
                Span::styled(" nav    ", desc),
                Span::styled("/", key),
                Span::styled("  search    ", desc),
                Span::styled("⏎", key),
                Span::styled(" view    ", desc),
                Span::styled("^R", key),
                Span::styled(" resume    ", desc),
                Span::styled("F5", key),
                Span::styled(" refresh    ", desc),
                Span::styled("?", key),
                Span::styled(" help", desc),
            ])
        }
        FooterState::Viewer => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("↑↓", key),
            Span::styled(" scroll    ", desc),
            Span::styled("/", key),
            Span::styled("  search    ", desc),
            Span::styled("e", key),
            Span::styled(" copy    ", desc),
            Span::styled("r", key),
            Span::styled(" rename    ", desc),
            Span::styled("q", key),
            Span::styled(" back    ", desc),
            Span::styled("?", key),
            Span::styled(" help", desc),
        ]),
        FooterState::ViewerMessageNav => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("J K", key),
            Span::styled(" message    ", desc),
            Span::styled("y", key),
            Span::styled(" copy message    ", desc),
            Span::styled("Esc", key),
            Span::styled(" exit nav    ", desc),
            Span::styled("?", key),
            Span::styled(" help", desc),
        ]),
        FooterState::ViewerSearchActive { current, total } => Line::from(vec![
            Span::styled("  ", desc),
            Span::styled("n", key),
            Span::styled(" next    ", desc),
            Span::styled("N", key),
            Span::styled(" prev    ", desc),
            Span::styled(format!("{} / {} matches", current, total), desc),
            Span::styled("    ", desc),
            Span::styled("Esc", key),
            Span::styled(" close", desc),
        ]),
    }
}

/// Returns true when the terminal is too small to comfortably show
/// the full list rows. In compact mode, list entries collapse from
/// three lines (header + preview + separator) to two (header +
/// separator, no preview), and the footer shows fewer hints.
fn is_compact_layout(area: Rect) -> bool {
    area.height < 20 || area.width < 60
}

/// Render the search-input band as a single styled line:
///   `  search ▸ <query>` (or, if query is empty, with a placeholder).
pub fn search_line(theme: &Theme, query: &str) -> Line<'static> {
    use ratatui::style::Style;
    use ratatui::text::Span;

    let label = Style::default().fg(rgb(theme.text_muted));
    let arrow = Style::default().fg(rgb(theme.accent));
    let primary = Style::default().fg(rgb(theme.text_primary));
    let placeholder = Style::default().fg(rgb(theme.text_tertiary));

    let mut spans = vec![
        Span::styled("  search ", label),
        Span::styled("▸", arrow),
        Span::styled(" ", label),
    ];
    if query.is_empty() {
        spans.push(Span::styled("(fuzzy across all transcripts)", placeholder));
    } else {
        spans.push(Span::styled(query.to_string(), primary));
    }
    Line::from(spans)
}

/// Split `text` into spans where any whitespace-separated word of
/// `query` matches (case-insensitive). Matched substrings carry
/// `accent_style`; everything else carries `base_style`. Returns a
/// single base-styled span when `query` is empty or has no hits.
///
/// Match strategy: each whitespace-separated query word is independently
/// substring-matched against `text` (case-insensitive). All non-overlapping
/// matches across all words are highlighted. Mirrors the search-AND
/// semantics elsewhere in the app: every word that matches gets surfaced.
pub fn highlight_query(
    text: &str,
    query: &str,
    base_style: Style,
    accent_style: Style,
) -> Vec<Span<'static>> {
    use ratatui::text::Span;

    if query.trim().is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let lower_text = text.to_lowercase();
    let words: Vec<String> = query
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    // Collect all (start, end) match ranges from every word.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for word in &words {
        let mut search_from = 0;
        while let Some(pos) = lower_text[search_from..].find(word.as_str()) {
            let start = search_from + pos;
            let end = start + word.len();
            ranges.push((start, end));
            search_from = end;
        }
    }
    if ranges.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    // Sort and merge overlapping ranges.
    ranges.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in ranges {
        if let Some(last) = merged.last_mut()
            && s <= last.1
        {
            last.1 = last.1.max(e);
            continue;
        }
        merged.push((s, e));
    }

    // Build spans: alternating base + accent over the byte ranges.
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut cursor = 0;
    for (s, e) in merged {
        if s > cursor {
            spans.push(Span::styled(text[cursor..s].to_string(), base_style));
        }
        spans.push(Span::styled(text[s..e].to_string(), accent_style));
        cursor = e;
    }
    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), base_style));
    }
    spans
}

/// Render a single conversation entry inside the framed list.
///
/// Normal mode returns 3 styled lines: header (project + title +
/// right-aligned `<N>msg · <age>`), preview (the conversation's last
/// user question), separator (thin `─` rule). Compact mode returns 2
/// lines (header + separator, no preview).
///
/// `inner_width` is the column width inside the outer frame (i.e.
/// `inner_area.width`), used to right-align the metadata and to
/// truncate the preview/separator to fit.
///
/// Selection is indicated by a gutter bar `▌` in the header row's
/// leftmost glyph column; the bg-tint highlight is applied by the
/// caller via `List::highlight_style`.
#[allow(clippy::too_many_arguments)]
pub fn list_row_lines(
    theme: &Theme,
    conv: &crate::history::Conversation,
    selected: bool,
    query: &str,
    compact: bool,
    inner_width: u16,
    id: usize,
    id_width: usize,
) -> Vec<Line<'static>> {
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Span;

    // Styles
    let gutter_style = Style::default().fg(rgb(theme.accent));
    let project_style = Style::default().fg(rgb(theme.text_muted));
    let title_style = if selected {
        Style::default()
            .fg(rgb(theme.text_primary))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(rgb(theme.text_primary))
    };
    let dim = Style::default().fg(rgb(theme.text_muted));
    let separator_style = Style::default().fg(rgb(theme.separator));
    let accent_hl = Style::default()
        .fg(rgb(theme.accent))
        .add_modifier(Modifier::BOLD);

    // ── Row 1 — header ──────────────────────────────────────────
    let project = conv.project_name.as_deref().unwrap_or("?");
    let title = conv
        .custom_title
        .as_deref()
        .or(conv.summary.as_deref())
        .unwrap_or("(no title)");

    let (age, _) = format_timestamp(conv.timestamp, chrono::Local::now());
    let metadata = format!("{}msg · {}", conv.message_count, age);

    // ID column: right-aligned numeral, fixed width = id_width.
    let id_text = format!("{:>width$}", id, width = id_width);

    // Layout widths inside the inner area:
    //   id (id_width) + gap (1) + glyph (1) + gap (1) + project (10) + gap (2)
    //   = id_width + 15
    let left_width: usize = id_width + 15;
    const RIGHT_MARGIN: usize = 1;

    let glyph = if selected { "▌" } else { " " };

    // Build the highlighted title spans first (search-as-you-type accent).
    let title_spans = highlight_query(title, query, title_style, accent_hl);

    let inner = inner_width as usize;
    let meta_width = unicode_width::UnicodeWidthStr::width(metadata.as_str());
    let title_max = inner.saturating_sub(left_width + meta_width + RIGHT_MARGIN + 1);
    let title_visible = unicode_width::UnicodeWidthStr::width(title).min(title_max);

    let used = left_width + title_visible;
    let padding = inner
        .saturating_sub(used + meta_width + RIGHT_MARGIN)
        .max(1);

    let mut header_spans: Vec<Span<'static>> = vec![
        Span::styled(id_text, dim),       // NEW — ID column, dim
        Span::styled(" ", project_style), // 1-col gap after ID
        Span::styled(glyph.to_string(), gutter_style),
        Span::styled(" ", project_style),
        Span::styled(format!("{:<10}", project), project_style),
        Span::styled("  ", project_style),
    ];
    header_spans.extend(truncate_spans(title_spans, title_max));
    header_spans.push(Span::styled(" ".repeat(padding), dim));
    header_spans.push(Span::styled(metadata.clone(), dim));
    let header_line = Line::from(header_spans);

    // Separator indent and right-margin computation — shared by both
    // compact and normal-mode separators.
    let sep_indent = id_width + 2;

    if compact {
        let sep_line = Line::from(vec![
            Span::styled(" ".repeat(sep_indent), separator_style),
            Span::styled(
                "─".repeat(inner.saturating_sub(sep_indent + 2)),
                separator_style,
            ),
        ]);
        return vec![header_line, sep_line];
    }

    // ── Row 2 — preview ──────────────────────────────────────────
    let preview_indent = id_width + 4;
    let preview_max = inner.saturating_sub(preview_indent + RIGHT_MARGIN);
    let preview_text = conv.last_user_question.as_deref().unwrap_or("");
    let preview_truncated = truncate_str(preview_text, preview_max);
    let preview_line = Line::from(vec![
        Span::styled(" ".repeat(preview_indent), dim),
        Span::styled(preview_truncated, dim),
    ]);

    // ── Row 3 — separator ───────────────────────────────────────
    let sep_line = Line::from(vec![
        Span::styled(" ".repeat(sep_indent), separator_style),
        Span::styled(
            "─".repeat(inner.saturating_sub(sep_indent + 2)),
            separator_style,
        ),
    ]);

    vec![header_line, preview_line, sep_line]
}

/// Truncate a string to `max_cols` display columns (not bytes, not
/// code points). Appends `…` if truncated. Returns an empty string
/// when `max_cols == 0`.
fn truncate_str(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let total = unicode_width::UnicodeWidthStr::width(s);
    if total <= max_cols {
        return s.to_string();
    }
    // Reserve 1 col for the ellipsis (`…` is 1 col).
    let keep_cols = max_cols.saturating_sub(1);
    let mut out = String::new();
    let mut taken = 0;
    for ch in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if taken + w > keep_cols {
            break;
        }
        out.push(ch);
        taken += w;
    }
    out.push('…');
    out
}

/// Truncate a sequence of styled spans to a total display width of
/// `max_cols`. Walks the spans left-to-right and appends `…` if any
/// content was dropped.
fn truncate_spans(spans: Vec<Span<'static>>, max_cols: usize) -> Vec<Span<'static>> {
    if max_cols == 0 {
        return vec![];
    }
    let total: usize = spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    if total <= max_cols {
        return spans;
    }
    // Reserve 1 col for the ellipsis.
    let keep_cols = max_cols.saturating_sub(1);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut taken = 0;
    for span in spans {
        let span_cols = unicode_width::UnicodeWidthStr::width(span.content.as_ref());
        if taken + span_cols <= keep_cols {
            out.push(span);
            taken += span_cols;
        } else {
            // Partial take from this span.
            let remaining = keep_cols.saturating_sub(taken);
            if remaining > 0 {
                let mut chunk = String::new();
                let mut chunk_cols = 0;
                for ch in span.content.chars() {
                    let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                    if chunk_cols + w > remaining {
                        break;
                    }
                    chunk.push(ch);
                    chunk_cols += w;
                }
                if !chunk.is_empty() {
                    out.push(Span::styled(chunk, span.style));
                }
            }
            break;
        }
    }
    let ellipsis_style = out.last().map(|s| s.style).unwrap_or_default();
    out.push(Span::styled("…", ellipsis_style));
    out
}

/// Duration before status messages auto-clear
const STATUS_TTL: std::time::Duration = std::time::Duration::from_secs(3);

/// Format model name for display (e.g., "claude-opus-4-5-20251101" → "opus-4.5")
#[allow(dead_code)]
fn format_model_name(model: &str) -> String {
    // Handle claude-opus-4-5-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-opus-4-5-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "opus-4.5".to_string();
    }

    // Handle claude-sonnet-4-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-sonnet-4-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "sonnet-4".to_string();
    }

    // Handle claude-3-5-sonnet-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-3-5-sonnet-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "sonnet-3.5".to_string();
    }

    // Handle claude-3-5-haiku-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-3-5-haiku-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "haiku-3.5".to_string();
    }

    // Handle claude-3-opus-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-3-opus-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "opus-3".to_string();
    }

    // Handle claude-3-sonnet-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-3-sonnet-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "sonnet-3".to_string();
    }

    // Handle claude-3-haiku-YYYYMMDD format
    if let Some(rest) = model.strip_prefix("claude-3-haiku-")
        && rest.chars().all(|c| c.is_ascii_digit())
    {
        return "haiku-3".to_string();
    }

    // Unknown format - truncate if too long
    if model.len() > 20 {
        format!("{}…", &model[..19])
    } else {
        model.to_string()
    }
}

/// Format token count with K/M suffix (short form, e.g., "926k")
#[allow(dead_code)]
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{}k", tokens / 1_000)
    } else {
        tokens.to_string()
    }
}

/// Format token count with K/M suffix and "tokens" label (long form, e.g., "926k tokens")
#[allow(dead_code)]
fn format_tokens_long(tokens: u64) -> String {
    format!("{} tokens", format_tokens(tokens))
}

/// Render the TUI
pub fn render(frame: &mut Frame, app: &App) {
    match app.app_mode() {
        AppMode::List => render_list_mode(frame, app),
        AppMode::View(state) => render_view_mode(frame, app, state),
    }
}

/// Render the list mode (conversation browser)
fn render_list_mode(frame: &mut Frame, app: &App) {
    let theme = th();
    let area = frame.area();
    let compact = is_compact_layout(area);

    // Header line (idle / search / loading)
    let scope = if app.workspace_filter() && app.has_project_context() {
        match app.current_project_dir_name() {
            Some(encoded) => {
                let path = crate::history::decode_project_dir_name_to_path(encoded);
                let name = crate::history::format_short_name_from_path(&path);
                format!("this project: {}", name)
            }
            None => "this project".to_string(),
        }
    } else {
        "all projects".to_string()
    };
    let header_state = if app.is_loading() {
        HeaderState::Loading {
            scope: &scope,
            so_far: app.conversations().len(),
        }
    } else if !app.query().is_empty() {
        HeaderState::Search {
            scope: &scope,
            matched: app.filtered().len(),
            total: app.conversations().len(),
        }
    } else {
        HeaderState::Idle {
            scope: &scope,
            total: app.conversations().len(),
        }
    };
    let header_title = header_line(theme, &header_state);

    // Footer line (key hints, or status message)
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ => FooterState::ListIdle,
    };
    let footer_title = footer_line(theme, &footer_state, compact);

    // Outer block: rounded, accent-dim border, header in top title, footer
    // key hints in bottom title.
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(header_title)
        .title_bottom(footer_title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Inside the frame: search input row + top rule + list.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search input
            Constraint::Length(1), // top rule
            Constraint::Min(0),    // list
        ])
        .split(inner);

    // Search input — only RENDERS the styled line; cursor must be
    // positioned separately.
    frame.render_widget(Paragraph::new(search_line(theme, app.query())), chunks[0]);

    // Cursor at the end of the query. Prefix "  search ▸ " is 11 cols.
    let prefix_cols: u16 = 11;
    let cols_before_cursor: usize = app
        .query()
        .chars()
        .take(app.cursor_pos())
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum();
    let max_x = chunks[0].x + chunks[0].width.saturating_sub(1);
    let cursor_x =
        (chunks[0].x + prefix_cols + cols_before_cursor.min(u16::MAX as usize) as u16).min(max_x);
    frame.set_cursor_position(Position::new(cursor_x, chunks[0].y));

    // Top rule (between search and list — visually separates them)
    frame.render_widget(horizontal_rule(theme, chunks[1].width), chunks[1]);

    // List
    let mut list_state = app.list_state();
    render_list(frame, app, chunks[2], compact, &mut list_state);

    // Confirm-delete and Help still render on top.
    if *app.dialog_mode() == DialogMode::ConfirmDelete {
        render_confirm_dialog(frame, inner);
    }
    if *app.dialog_mode() == DialogMode::Help {
        render_help_overlay(
            frame,
            false,
            false,
            app.keys(),
            app.default_skip_permissions(),
        );
    }
}

/// Render a thin horizontal rule across `width` columns.
fn horizontal_rule(theme: &Theme, width: u16) -> Paragraph<'static> {
    let rule = "─".repeat(width as usize);
    Paragraph::new(Span::styled(
        rule,
        Style::default().fg(rgb(theme.separator)),
    ))
}

/// Render the view mode (conversation viewer)
fn render_view_mode(frame: &mut Frame, app: &App, state: &ViewState) {
    let theme = th();
    let area = frame.area();
    let compact = is_compact_layout(area);

    // Header: ◈ ccmanager · session <12-char> · N turns
    let conv = app
        .conversations()
        .iter()
        .find(|c| c.path == state.conversation_path);
    let message_count = conv.map(|c| c.message_count).unwrap_or(0);
    let session_id_short = state
        .conversation_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| stem.chars().take(12).collect::<String>())
        .unwrap_or_else(|| "?".to_string());
    let scope = format!("session {}", session_id_short);
    let header_state = HeaderState::Idle {
        scope: &scope,
        total: message_count,
    };
    let header_title =
        override_trailing_word(header_line(theme, &header_state), "sessions", "turns");

    // Footer: status message wins; otherwise viewer / message-nav variant.
    let footer_state = match app.status_message() {
        Some((msg, instant)) if instant.elapsed() < STATUS_TTL => {
            FooterState::StatusMessage(msg.as_str())
        }
        _ if state.search_mode == ViewSearchMode::Active && !state.search_matches.is_empty() => {
            FooterState::ViewerSearchActive {
                current: state.current_match + 1,
                total: state.search_matches.len(),
            }
        }
        _ if state.message_nav_active => FooterState::ViewerMessageNav,
        _ => FooterState::Viewer,
    };
    let footer_title = footer_line(theme, &footer_state, compact);

    // Outer block
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(header_title)
        .title_bottom(footer_title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Find bar (when search-typing): takes the last content row.
    let search_active = state.search_mode == ViewSearchMode::Typing;
    if search_active {
        let constraints = [Constraint::Min(0), Constraint::Length(1)];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);
        render_view_content(frame, state, chunks[0]);
        render_search_input(frame, state, chunks[1]);
    } else {
        render_view_content(frame, state, inner);
    }

    // Dialog overlays stay the same.
    match app.dialog_mode() {
        DialogMode::ConfirmDelete => render_confirm_dialog(frame, inner),
        DialogMode::YankMenu { selected } => render_export_menu(frame, *selected),
        DialogMode::RenameInput { buffer } => render_rename_dialog(frame, buffer),
        DialogMode::Help => render_help_overlay(
            frame,
            true,
            app.is_single_file_mode(),
            app.keys(),
            app.default_skip_permissions(),
        ),
        DialogMode::None => {}
    }
}

/// Replace the last `Span` of a `Line` whose text ends with `from_suffix`
/// with one that ends with `to_suffix` instead. Used by viewer-mode
/// header to render "turns" instead of "sessions".
fn override_trailing_word(
    mut line: Line<'static>,
    from_suffix: &str,
    to_suffix: &str,
) -> Line<'static> {
    if let Some(last) = line.spans.last_mut()
        && last.content.ends_with(from_suffix)
    {
        let s = last.content.to_string();
        let new = s.trim_end_matches(from_suffix).to_string() + to_suffix;
        last.content = new.into();
    }
    line
}

fn render_view_content(frame: &mut Frame, state: &ViewState, area: Rect) {
    let visible_height = area.height as usize;
    let query_lower = state.search_query.to_lowercase();

    // Determine focused message line range (only when nav mode active)
    let focused_range = if state.message_nav_active {
        state
            .focused_message
            .and_then(|idx| state.message_ranges.get(idx))
            .map(|m| m.start_line..m.end_line)
    } else {
        None
    };

    let visible_lines: Vec<Line> = state
        .rendered_lines
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(visible_height)
        .map(|(line_idx, rendered)| {
            let is_current_match = state.search_matches.get(state.current_match) == Some(&line_idx);
            let has_match = !query_lower.is_empty() && state.search_matches.contains(&line_idx);

            let is_focused = focused_range
                .as_ref()
                .is_some_and(|r| r.contains(&line_idx));

            // Gutter indicator (only shown in message nav mode)
            let gutter = if state.message_nav_active {
                if is_focused {
                    Span::styled("▌ ", Style::default().fg(rgb(th().accent)))
                } else {
                    Span::raw("  ")
                }
            } else {
                Span::raw("")
            };

            let mut spans: Vec<Span> = vec![gutter];

            if has_match && !query_lower.is_empty() {
                spans.extend(highlight_line_matches(
                    rendered,
                    &query_lower,
                    is_current_match,
                ));
            } else {
                spans.extend(
                    rendered
                        .spans
                        .iter()
                        .map(|(text, style)| styled_span(text, style)),
                );
            }

            Line::from(spans)
        })
        .collect();

    let content = Paragraph::new(visible_lines);
    frame.render_widget(content, area);
}

fn render_search_input(frame: &mut Frame, state: &ViewState, area: Rect) {
    let theme = th();
    let label = Style::default().fg(rgb(theme.text_muted));
    let arrow = Style::default().fg(rgb(theme.accent));
    let primary = Style::default().fg(rgb(theme.text_primary));

    let match_info = if state.search_matches.is_empty() {
        if state.search_query.is_empty() {
            String::new()
        } else {
            "     no matches".to_string()
        }
    } else {
        format!(
            "     {} / {} matches",
            state.current_match + 1,
            state.search_matches.len()
        )
    };

    let input_line = Line::from(vec![
        Span::styled("  find ", label),
        Span::styled("▸", arrow),
        Span::styled(" ", label),
        Span::styled(state.search_query.clone(), primary),
        Span::styled(match_info, label),
    ]);

    let input = Paragraph::new(input_line);
    frame.render_widget(input, area);

    // Position cursor at end of query. Prefix "  find ▸ " has display width of 9
    // ASCII columns plus the 1-column ▸ glyph = 9 columns of leading text.
    // ("  " = 2) + ("find" = 4) + (" " = 1) + ("▸" = 1) + (" " = 1) = 9
    let prefix_width: u16 = 9;
    let query_width: usize = state
        .search_query
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum();
    let max_x = area.x + area.width.saturating_sub(1);
    let cursor_x = (area.x + prefix_width + query_width.min(u16::MAX as usize) as u16).min(max_x);
    frame.set_cursor_position(Position::new(cursor_x, area.y));
}

/// Highlight search matches across the full line text, handling matches that span
/// across multiple styled spans. Works by finding match positions in the concatenated
/// line text, then rebuilding spans with highlights applied at the correct positions.
fn highlight_line_matches(
    rendered: &RenderedLine,
    query: &str,
    is_current_match: bool,
) -> Vec<Span<'static>> {
    // Concatenate all span texts to get the full line
    let full_text: String = rendered
        .spans
        .iter()
        .map(|(text, _)| text.as_str())
        .collect();
    let full_lower = full_text.to_lowercase();

    // Find match positions using char indices to safely handle Unicode
    // (lowercasing can change byte lengths for some characters)
    let orig_chars: Vec<(usize, char)> = full_text.char_indices().collect();
    let lower_chars: Vec<char> = full_lower.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();

    let mut match_byte_ranges: Vec<(usize, usize)> = Vec::new();
    if !query_chars.is_empty() {
        let mut i = 0;
        while i + query_chars.len() <= lower_chars.len() {
            if lower_chars[i..i + query_chars.len()] == query_chars[..] {
                // Guard against Unicode casing expansion (e.g. ß → ss) where
                // lower_chars may be longer than orig_chars
                if i >= orig_chars.len() {
                    break;
                }
                let start_byte = orig_chars[i].0;
                let end_byte = if i + query_chars.len() < orig_chars.len() {
                    orig_chars[i + query_chars.len()].0
                } else {
                    full_text.len()
                };
                match_byte_ranges.push((start_byte, end_byte));
                i += query_chars.len();
            } else {
                i += 1;
            }
        }
    }

    if match_byte_ranges.is_empty() {
        return rendered
            .spans
            .iter()
            .map(|(t, s)| styled_span(t, s))
            .collect();
    }

    let match_style = if is_current_match {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    } else {
        Style::default()
            .bg(rgb(th().search_match_bg))
            .fg(Color::Black)
    };

    // Build output spans by walking through original spans and splitting at match boundaries
    let mut result: Vec<Span<'static>> = Vec::new();
    let mut match_idx = 0;
    let mut global_offset: usize = 0;

    for (text, style) in &rendered.spans {
        let span_start = global_offset;
        let span_end = global_offset + text.len();
        let base_style = build_style(style);
        let mut pos = span_start;

        while pos < span_end {
            // Skip past matches that are entirely before our position
            while match_idx < match_byte_ranges.len() && match_byte_ranges[match_idx].1 <= pos {
                match_idx += 1;
            }

            if match_idx < match_byte_ranges.len() {
                let (ms, me) = match_byte_ranges[match_idx];
                if pos >= ms && pos < me {
                    // Inside a match
                    let end = me.min(span_end);
                    result.push(Span::styled(full_text[pos..end].to_string(), match_style));
                    pos = end;
                } else if ms < span_end {
                    // There's a match starting within this span, emit text before it
                    let end = ms.min(span_end);
                    if end > pos {
                        result.push(Span::styled(full_text[pos..end].to_string(), base_style));
                    }
                    pos = end;
                } else {
                    // No more matches in this span
                    result.push(Span::styled(
                        full_text[pos..span_end].to_string(),
                        base_style,
                    ));
                    pos = span_end;
                }
            } else {
                // No more matches at all
                result.push(Span::styled(
                    full_text[pos..span_end].to_string(),
                    base_style,
                ));
                pos = span_end;
            }
        }

        global_offset = span_end;
    }

    result
}

fn build_style(style: &LineStyle) -> Style {
    let mut s = Style::default();
    if let Some((r, g, b)) = style.fg {
        s = s.fg(Color::Rgb(r, g, b));
    }
    if style.bold {
        s = s.bold();
    }
    if style.italic {
        s = s.italic();
    }
    if style.dimmed {
        s = s.fg(rgb(th().text_muted));
    }
    s
}

fn styled_span(text: &str, style: &LineStyle) -> Span<'static> {
    Span::styled(text.to_string(), build_style(style))
}

/// Layout for the small modal popovers (confirm, export menu, rename,
/// help overlay). Caller provides the title, the body (a list of lines),
/// and the dismiss hint that goes into the bottom border.
///
/// The modal is centered on `frame.area()` and sized to `(width, height)`.
/// Border is rounded, accent-dim color; title is full accent.
fn render_modal(
    frame: &mut Frame,
    title: &str,
    body: Vec<Line<'static>>,
    hint: &str,
    width: u16,
    height: u16,
) {
    use ratatui::style::Modifier;

    let theme = th();
    let area = frame.area();
    // Clamp to the visible frame so we never write past the buffer.
    let width = width.min(area.width);
    let height = height.min(area.height);
    let rect = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(rgb(theme.accent_dim)))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title.to_string(),
                Style::default()
                    .fg(rgb(theme.accent))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_bottom(Line::from(vec![
            Span::raw(" "),
            Span::styled(hint.to_string(), Style::default().fg(rgb(theme.text_muted))),
            Span::raw(" "),
        ]));

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let body_para = Paragraph::new(body);
    frame.render_widget(body_para, inner);
}

fn render_confirm_dialog(frame: &mut Frame, area: Rect) {
    use ratatui::style::Modifier;

    let _ = area; // render_modal computes its own area; signature kept for callers
    let theme = th();
    let body = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Delete this conversation?",
                Style::default().fg(rgb(theme.text_primary)),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  \u{25c6} ", Style::default().fg(rgb(theme.accent))),
            Span::styled(
                "y",
                Style::default()
                    .fg(rgb(theme.text_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  yes, delete",
                Style::default().fg(rgb(theme.text_primary)),
            ),
        ]),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("n", Style::default().fg(rgb(theme.text_muted))),
            Span::styled("  no, cancel", Style::default().fg(rgb(theme.text_muted))),
        ]),
    ];
    render_modal(
        frame,
        "Confirm delete",
        body,
        "y confirm  \u{00b7}  n / Esc cancel",
        46,
        7,
    );
}

fn render_export_menu(frame: &mut Frame, selected: usize) {
    use ratatui::style::Modifier;

    let theme = th();
    let options = [
        "[1] Ledger (formatted)",
        "[2] Plain text",
        "[3] Markdown",
        "[4] JSONL (raw)",
    ];

    let mut body = Vec::new();
    body.push(Line::from(""));
    for (i, opt) in options.iter().enumerate() {
        let row_style = if i == selected {
            Style::default()
                .bg(rgb(theme.selection_bg))
                .fg(rgb(theme.text_primary))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(rgb(theme.text_primary))
        };
        let glyph = if i == selected { "\u{25c6} " } else { "  " };
        body.push(Line::from(vec![
            Span::styled(
                format!("  {}", glyph),
                Style::default().fg(rgb(theme.accent)),
            ),
            Span::styled(opt.to_string(), row_style),
        ]));
    }

    render_modal(
        frame,
        "Copy to clipboard",
        body,
        "\u{2191}\u{2193} select  \u{00b7}  \u{23ce} confirm  \u{00b7}  Esc cancel",
        50,
        7,
    );
}

fn render_rename_dialog(frame: &mut Frame, buffer: &str) {
    let theme = th();
    let area = frame.area();
    // Width scales with buffer length so long titles stay visible; clamped so
    // the modal doesn't hug the terminal edges.
    let min_width: u16 = 60;
    let desired = (buffer.chars().count() as u16 + 6).max(min_width);
    let modal_width = desired.min(area.width.saturating_sub(4).max(min_width));
    let modal_height: u16 = 6;

    // Show the input buffer with a trailing underscore cursor. If empty,
    // the underscore alone signals where text will appear.
    let input_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            buffer.to_string(),
            Style::default().fg(rgb(theme.custom_title)),
        ),
        Span::styled("_", Style::default().fg(rgb(theme.accent))),
    ]);

    let body = vec![Line::from(""), input_line, Line::from("")];

    render_modal(
        frame,
        "Rename conversation",
        body,
        "\u{23ce} save  \u{00b7}  Esc cancel  \u{00b7}  (empty = clear title)",
        modal_width,
        modal_height,
    );
}

fn render_help_overlay(
    frame: &mut Frame,
    is_view_mode: bool,
    is_single_file_mode: bool,
    keys: &KeyBindings,
    default_skip_permissions: bool,
) {
    let exit_text = if is_single_file_mode {
        "Quit"
    } else {
        "Back to list"
    };

    // Label the two resume bindings so the user always knows which one
    // skips permissions vs prompts. Whichever is configured as primary gets
    // the "skip permissions" label by default.
    let (primary_resume_label, alt_resume_label, primary_fork_label, alt_fork_label) =
        if default_skip_permissions {
            (
                "Resume (skip permissions)",
                "Resume (with prompts)",
                "Fork resume (skip permissions)",
                "Fork resume (with prompts)",
            )
        } else {
            (
                "Resume (with prompts)",
                "Resume (skip permissions)",
                "Fork resume (with prompts)",
                "Fork resume (skip permissions)",
            )
        };

    let shortcuts: Vec<(String, &str)> = if is_view_mode {
        vec![
            ("j / \u{2193}".into(), "Scroll down"),
            ("k / \u{2191}".into(), "Scroll up"),
            ("J / ]".into(), "Next message"),
            ("K / [".into(), "Previous message"),
            ("d / Ctrl+D".into(), "Half page down"),
            ("u / Ctrl+U".into(), "Half page up"),
            ("g / Home".into(), "Jump to top"),
            ("G / End".into(), "Jump to bottom"),
            ("/".into(), "Search"),
            ("n / N".into(), "Next / prev match"),
            ("t".into(), "Cycle tools: off/trunc/full"),
            ("T".into(), "Toggle thinking"),
            ("i".into(), "Toggle timing"),
            ("Q".into(), "Toggle questions-only view"),
            ("r".into(), "Rename conversation"),
            ("e".into(), "Copy conversation to clipboard"),
            ("y".into(), "Copy to clipboard / message"),
            ("p".into(), "Show file path"),
            ("Y".into(), "Copy path"),
            ("I".into(), "Copy session ID"),
            ("F5".into(), "Reload list + current viewer from disk"),
            (keys.resume.help_label(), primary_resume_label),
            (keys.resume_alt.help_label(), alt_resume_label),
            (keys.fork.help_label(), primary_fork_label),
            (keys.fork_alt.help_label(), alt_fork_label),
            (keys.delete.help_label(), "Delete"),
            ("q / Esc".into(), exit_text),
        ]
    } else {
        vec![
            ("\u{2191} / \u{2193}".into(), "Move selection"),
            ("\u{2190} / \u{2192}".into(), "Move cursor"),
            ("Ctrl+P / N".into(), "Move selection"),
            ("Ctrl+D".into(), "Half page down"),
            ("Ctrl+U".into(), "Kill to start of line"),
            ("Ctrl+K".into(), "Kill to end of line"),
            ("PgUp / PgDn".into(), "Jump by page"),
            ("Home / End".into(), "Jump to first/last"),
            ("Tab".into(), "Toggle scope (All/Project)"),
            ("Enter".into(), "Open viewer"),
            ("Ctrl+O".into(), "Select and exit"),
            ("Ctrl+W".into(), "Delete word"),
            ("F5".into(), "Reload conversation list from disk"),
            (keys.resume.help_label(), primary_resume_label),
            (keys.resume_alt.help_label(), alt_resume_label),
            (keys.fork.help_label(), primary_fork_label),
            (keys.fork_alt.help_label(), alt_fork_label),
            (keys.delete.help_label(), "Delete"),
            ("Esc".into(), "Quit"),
        ]
    };

    let area = frame.area();
    // Calculate dimensions based on content (use chars().count() for Unicode)
    let max_key_len = shortcuts
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let max_action_len = shortcuts
        .iter()
        .map(|(_, a)| a.chars().count())
        .max()
        .unwrap_or(0);
    // Padding: 2 chars left + key + " │ " (3) + action + 2 chars right
    let content_width = (max_key_len + max_action_len + 11) as u16;
    // Height: 1 top padding + shortcuts + 2 border (title_bottom counts as border row)
    let content_height = shortcuts.len() as u16 + 3;

    // Clamp to the available frame so the modal Rect always stays inside
    // the buffer. Without this, a tall help overlay in a short terminal
    // panics ratatui's buffer access at the bottom edge. Content is then
    // clipped naturally by the Paragraph widget rather than panicking.
    let menu_width = content_width.min(area.width);
    let menu_height = content_height.min(area.height);

    // Build shortcut lines with padding
    let mut body: Vec<Line<'static>> = Vec::new();
    body.push(Line::from("")); // Top padding
    for (key, action) in &shortcuts {
        let key_padding = max_key_len - key.chars().count();
        body.push(Line::from(vec![
            Span::raw("  "), // Left padding
            Span::styled(
                format!("{}{}", key, " ".repeat(key_padding)),
                Style::default().fg(rgb(th().accent)),
            ),
            Span::styled(" \u{2502} ", Style::default().fg(rgb(th().border))),
            Span::styled(
                action.to_string(),
                Style::default().fg(rgb(th().text_primary)),
            ),
        ]));
    }

    render_modal(
        frame,
        "Shortcuts",
        body,
        "q / Esc close",
        menu_width,
        menu_height,
    );
}

fn render_list(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    compact: bool,
    list_state: &mut ListState,
) {
    let theme = th();
    let filtered = app.filtered();
    let selected_idx = app.selected();

    if filtered.is_empty() {
        let dim = Style::default().fg(rgb(theme.text_muted));
        let lines: Vec<Line<'static>> = if app.conversations().is_empty() && !app.is_loading() {
            // No history at all.
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "you don't have any Claude Code conversations yet",
                    dim,
                )),
            ]
        } else if !app.query().is_empty() {
            // No matches for the active search.
            vec![
                Line::from(""),
                Line::from(Span::styled("no conversations match your search", dim)),
                Line::from(Span::styled("press Esc to clear it", dim)),
            ]
        } else {
            // Loading and nothing arrived yet — let the header's "loading… N so far" carry the message.
            return;
        };
        let para =
            ratatui::widgets::Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(para, area);
        return;
    }

    let id_width = filtered.len().to_string().len().max(3);

    let items: Vec<ListItem<'static>> = filtered
        .iter()
        .enumerate()
        .map(|(i, &conv_idx)| {
            let conv = &app.conversations()[conv_idx];
            let lines = list_row_lines(
                theme,
                conv,
                Some(i) == selected_idx,
                app.query(),
                compact,
                area.width,
                i + 1, // 1-based ID
                id_width,
            );
            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(rgb(theme.selection_bg)));

    list_state.select(selected_idx);
    frame.render_stateful_widget(list, area, list_state);
}

/// Recency level for timestamp color grading
enum Recency {
    Now,
    Minutes,
    Hours,
    Days,
    Old,
}

/// Format a timestamp as relative time for recent entries, absolute for older ones.
/// Returns (formatted_string, recency) for color grading.
fn format_timestamp(timestamp: DateTime<Local>, now: DateTime<Local>) -> (String, Recency) {
    let age = now.signed_duration_since(timestamp);

    // Future timestamps (clock skew): show absolute
    if age.num_seconds() < 0 {
        return (timestamp.format("%b %d, %H:%M").to_string(), Recency::Old);
    }

    let seconds = age.num_seconds();
    let minutes = age.num_minutes();
    let hours = age.num_hours();

    if seconds < 60 {
        return ("just now".to_string(), Recency::Now);
    }
    if minutes < 60 {
        return (format!("{minutes} min ago"), Recency::Minutes);
    }
    if hours < 24 {
        return (
            format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" }),
            Recency::Hours,
        );
    }

    // Use calendar day difference for "yesterday" accuracy
    let day_diff = now
        .date_naive()
        .signed_duration_since(timestamp.date_naive())
        .num_days();
    if day_diff == 1 {
        return ("yesterday".to_string(), Recency::Days);
    }
    if day_diff < 7 {
        return (format!("{day_diff} days ago"), Recency::Days);
    }

    (timestamp.format("%b %d, %H:%M").to_string(), Recency::Old)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_model_name_opus_45() {
        assert_eq!(format_model_name("claude-opus-4-5-20251101"), "opus-4.5");
    }

    #[test]
    fn test_format_model_name_sonnet_4() {
        assert_eq!(format_model_name("claude-sonnet-4-20250514"), "sonnet-4");
    }

    #[test]
    fn test_format_model_name_sonnet_35() {
        assert_eq!(
            format_model_name("claude-3-5-sonnet-20241022"),
            "sonnet-3.5"
        );
    }

    #[test]
    fn test_format_model_name_haiku_35() {
        assert_eq!(format_model_name("claude-3-5-haiku-20241022"), "haiku-3.5");
    }

    #[test]
    fn test_format_model_name_opus_3() {
        assert_eq!(format_model_name("claude-3-opus-20240229"), "opus-3");
    }

    #[test]
    fn test_format_model_name_unknown() {
        assert_eq!(format_model_name("custom-model"), "custom-model");
    }

    #[test]
    fn test_format_model_name_truncates_long() {
        let long_name = "very-long-unknown-model-name-that-exceeds-limit";
        let formatted = format_model_name(long_name);
        // 19 chars + ellipsis (3 bytes in UTF-8)
        assert!(formatted.chars().count() <= 20);
        assert!(formatted.ends_with('…'));
    }

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1000), "1k");
        assert_eq!(format_tokens(417000), "417k");
        assert_eq!(format_tokens(999999), "999k");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.0M");
        assert_eq!(format_tokens(1_500_000), "1.5M");
        assert_eq!(format_tokens(12_345_678), "12.3M");
    }

    #[test]
    fn test_format_tokens_long() {
        assert_eq!(format_tokens_long(500), "500 tokens");
        assert_eq!(format_tokens_long(1000), "1k tokens");
        assert_eq!(format_tokens_long(926000), "926k tokens");
        assert_eq!(format_tokens_long(1_500_000), "1.5M tokens");
    }

    // ---------- help overlay regression ----------
    //
    // Regression for: ratatui panic "index outside of buffer" when the
    // viewer help overlay is opened in a short terminal. The fix clamps
    // the modal Rect to the frame area so it never extends past the
    // buffer's bottom edge.

    use crate::config::KeyBindings;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Drive `render_help_overlay` against a fake `TestBackend` of the given
    /// size. Asserts no panic. Caller picks the size to exercise edge cases.
    fn try_render_help(width: u16, height: u16, is_view_mode: bool, default_skip: bool) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let keys = KeyBindings::default();
        terminal
            .draw(|frame| {
                render_help_overlay(frame, is_view_mode, false, &keys, default_skip);
            })
            .expect("draw must not panic");
    }

    #[test]
    fn help_overlay_does_not_panic_in_short_viewer_terminal() {
        // Reproduces the user-reported failure: 106x26 terminal, viewer
        // mode, the help overlay's content (~30 lines including resume,
        // resume_alt, fork, fork_alt, delete) overflows the frame. Without
        // the clamp, ratatui panics on the first row past height.
        try_render_help(106, 26, true, true);
    }

    #[test]
    fn help_overlay_does_not_panic_in_short_list_terminal() {
        try_render_help(80, 20, false, true);
    }

    #[test]
    fn help_overlay_handles_extreme_smallness() {
        // Pathologically small frame — clamping must not produce a 0-area
        // Rect or anything else that trips ratatui.
        try_render_help(20, 5, true, true);
        try_render_help(10, 3, false, true);
    }

    #[test]
    fn help_overlay_handles_inverted_skip_permissions() {
        // Inverted config swaps the labels on the resume rows; total
        // shortcut count is identical, so this is mainly a smoke test
        // that no labelling branch slipped past the clamp.
        try_render_help(106, 26, true, false);
        try_render_help(106, 26, false, false);
    }

    #[test]
    fn help_overlay_renders_fully_in_a_tall_terminal() {
        // When the terminal IS tall enough, all content should render
        // without any clamping kicking in. Just a sanity check that the
        // happy path still works.
        try_render_help(120, 60, true, true);
        try_render_help(120, 60, false, true);
    }
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use crate::tui::theme::Theme;

    fn theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn idle_state_shows_total_count() {
        let line = header_line(
            &theme(),
            &HeaderState::Idle {
                scope: "all projects",
                total: 47,
            },
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("◈ ccmanager"));
        assert!(rendered.contains("all projects"));
        assert!(rendered.contains("47 sessions"));
    }

    #[test]
    fn search_active_shows_matched_fraction() {
        let line = header_line(
            &theme(),
            &HeaderState::Search {
                scope: "all projects",
                matched: 5,
                total: 47,
            },
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("5 / 47 sessions match"));
    }

    #[test]
    fn loading_state_shows_so_far_count() {
        let line = header_line(
            &theme(),
            &HeaderState::Loading {
                scope: "all projects",
                so_far: 12,
            },
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("loading… 12 so far"));
    }
}

#[cfg(test)]
mod footer_tests {
    use super::*;
    use crate::tui::theme::Theme;

    fn theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn list_idle_has_all_six_key_hints() {
        let line = footer_line(&theme(), &FooterState::ListIdle, false);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        for hint in ["↑↓", "/", "⏎", "^R", "F5", "?"] {
            assert!(
                rendered.contains(hint),
                "missing hint {:?} in {:?}",
                hint,
                rendered
            );
        }
    }

    #[test]
    fn viewer_state_omits_resume_and_refresh() {
        let line = footer_line(&theme(), &FooterState::Viewer, false);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!rendered.contains("^R"));
        assert!(!rendered.contains("F5"));
        assert!(rendered.contains("e copy"));
        assert!(rendered.contains("r rename"));
    }

    #[test]
    fn status_message_replaces_hints() {
        let line = footer_line(
            &theme(),
            &FooterState::StatusMessage("Refreshed: 47 conversations"),
            false,
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("Refreshed: 47 conversations"));
        assert!(!rendered.contains("↑↓"));
    }

    #[test]
    fn message_nav_swaps_in_y_copy_message() {
        let line = footer_line(&theme(), &FooterState::ViewerMessageNav, false);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("J K"));
        assert!(rendered.contains("y copy message"));
        assert!(!rendered.contains("e copy"));
    }

    #[test]
    fn list_idle_compact_omits_resume_and_refresh() {
        let line = footer_line(&theme(), &FooterState::ListIdle, /* compact = */ true);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // Compact retains: ↑↓, /, ⏎, ?
        for hint in ["↑↓", "/", "⏎", "?"] {
            assert!(rendered.contains(hint), "compact should keep {:?}", hint);
        }
        // Compact omits: ^R, F5, "resume", "refresh"
        assert!(!rendered.contains("^R"));
        assert!(!rendered.contains("F5"));
        assert!(!rendered.contains("resume"));
        assert!(!rendered.contains("refresh"));
    }

    #[test]
    #[allow(non_snake_case)]
    fn viewer_search_active_shows_n_N_and_count() {
        let line = footer_line(
            &theme(),
            &FooterState::ViewerSearchActive {
                current: 3,
                total: 12,
            },
            /* compact = */ false,
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            rendered.contains("n next"),
            "missing `n next`: {:?}",
            rendered
        );
        assert!(
            rendered.contains("N prev"),
            "missing `N prev`: {:?}",
            rendered
        );
        assert!(
            rendered.contains("3 / 12 matches"),
            "missing count: {:?}",
            rendered
        );
        assert!(
            rendered.contains("Esc close"),
            "missing Esc hint: {:?}",
            rendered
        );
    }
}

#[cfg(test)]
mod search_line_tests {
    use super::*;
    use crate::tui::theme::Theme;

    #[test]
    fn empty_query_shows_placeholder() {
        let line = search_line(&Theme::dark(), "");
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("search ▸"));
        assert!(rendered.contains("(fuzzy across all transcripts)"));
    }

    #[test]
    fn nonempty_query_omits_placeholder() {
        let line = search_line(&Theme::dark(), "deploy");
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("search ▸"));
        assert!(rendered.contains("deploy"));
        assert!(!rendered.contains("(fuzzy"));
    }
}

#[cfg(test)]
mod list_row_tests {
    use super::*;
    use crate::history::Conversation;
    use crate::tui::theme::Theme;
    use chrono::Local;

    // The Conversation struct does NOT derive Default. Build one
    // by hand, filling each pub field. See history/mod.rs for the
    // canonical shape.
    fn make_conv(project: &str, title: &str, mins_ago: i64, message_count: usize) -> Conversation {
        let ts = Local::now() - chrono::Duration::minutes(mins_ago);
        Conversation {
            path: std::path::PathBuf::from(format!("/fake/{}.jsonl", title)),
            index: 0,
            timestamp: ts,
            preview: String::new(),
            preview_first: String::new(),
            preview_last: String::new(),
            full_text: String::new(),
            search_text_lower: String::new(),
            project_name: Some(project.to_string()),
            project_path: None,
            cwd: None,
            message_count,
            parse_errors: Vec::new(),
            summary: Some(title.to_string()),
            custom_title: None,
            model: None,
            total_tokens: 0,
            duration_minutes: None,
            last_user_question: Some(format!("question text for {}", title)),
        }
    }

    #[test]
    fn normal_mode_returns_three_lines() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        assert_eq!(lines.len(), 3, "expected header + preview + separator");
    }

    #[test]
    fn compact_mode_returns_two_lines_no_preview() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ true,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        assert_eq!(lines.len(), 2, "expected header + separator only");
        let second: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !second.contains("question text"),
            "compact mode must not include preview text, got: {:?}",
            second
        );
    }

    #[test]
    fn preview_row_shows_last_user_question() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        let preview: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            preview.contains("question text for Add F5 refresh"),
            "preview line should contain the last user question, got: {:?}",
            preview
        );
        assert!(
            !preview.contains("You:"),
            "no You: prefix expected, got: {:?}",
            preview
        );
    }

    #[test]
    fn metadata_is_right_aligned_with_msg_count_and_age() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.contains("47msg"),
            "header missing msg count: {:?}",
            header
        );
        assert!(header.contains("·"), "header missing middle-dot separator");
        let title_pos = header.find("Add F5 refresh").expect("title");
        let msg_pos = header.find("47msg").expect("msg count");
        assert!(title_pos < msg_pos, "metadata should follow the title");
        let display_width = header.chars().count();
        assert!(
            display_width <= 80,
            "header too wide ({}): {:?}",
            display_width,
            header
        );
    }

    #[test]
    fn selected_row_has_gutter_bar() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ true,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.contains("▌"),
            "selected row's header must contain the gutter bar ▌, got: {:?}",
            header
        );
    }

    #[test]
    fn cjk_title_metadata_still_fits_inner_width() {
        let mut conv = make_conv("ccmanager", "深入学习强化学习", 120, 47);
        // Force a CJK title via custom_title (summary fallback would also work).
        conv.custom_title = Some("深入学习强化学习".to_string());
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        let display_width = unicode_width::UnicodeWidthStr::width(header.as_str());
        assert!(
            display_width <= 80,
            "header overflows on CJK title: {} cols, content {:?}",
            display_width,
            header
        );
    }

    #[test]
    fn list_row_id_renders_right_aligned_in_3_cols() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 1,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.starts_with("  1 "),
            "expected `\"  1 \"` prefix (right-aligned ID, then space), got: {:?}",
            header
        );
    }

    #[test]
    fn list_row_id_two_digit() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 42,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.starts_with(" 42 "),
            "expected `\" 42 \"` prefix, got: {:?}",
            header
        );
    }

    #[test]
    fn list_row_id_three_digit() {
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 999,
            /* id_width = */ 3,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.starts_with("999 "),
            "expected `\"999 \"` prefix, got: {:?}",
            header
        );
    }

    #[test]
    fn list_row_id_minimum_width_one() {
        // When `id_width` is 1, the ID column produces a single-char numeral
        // followed by the 1-col gap, then the gutter / project / title.
        // This exercises the layout math at its minimum meaningful width.
        let conv = make_conv("ccmanager", "Add F5 refresh", 120, 47);
        let lines = list_row_lines(
            &Theme::dark(),
            &conv,
            /* selected = */ false,
            /* query    = */ "",
            /* compact  = */ false,
            /* inner_width = */ 80,
            /* id       = */ 7,
            /* id_width = */ 1,
        );
        let header: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            header.starts_with("7 "),
            "expected `\"7 \"` prefix at id_width=1, got: {:?}",
            header
        );
        // Verify the header still fits within inner_width.
        let display_width = unicode_width::UnicodeWidthStr::width(header.as_str());
        assert!(
            display_width <= 80,
            "header overflows at id_width=1: {} cols, content: {:?}",
            display_width,
            header
        );
    }
}

#[cfg(test)]
mod highlight_query_tests {
    use super::*;
    use ratatui::style::Style;

    #[test]
    fn empty_query_returns_single_base_span() {
        let base = Style::default();
        let accent = Style::default();
        let spans = highlight_query("Hello world", "", base, accent);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "Hello world");
    }

    #[test]
    fn single_word_matches_case_insensitive() {
        let base = Style::default();
        let accent = Style::default().fg(ratatui::style::Color::Red);
        let spans = highlight_query("Deploy strategy", "deploy", base, accent);
        // expect 2 spans: "Deploy" (accent) + " strategy" (base)
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "Deploy");
        assert_eq!(spans[0].style.fg, Some(ratatui::style::Color::Red));
        assert_eq!(spans[1].content.as_ref(), " strategy");
    }

    #[test]
    fn multiple_words_independently_highlighted() {
        let base = Style::default();
        let accent = Style::default().fg(ratatui::style::Color::Red);
        let spans = highlight_query("Add F5 refresh", "add refresh", base, accent);
        // matches: "Add" + " F5 " + "refresh"
        // spans: ["Add" accent, " F5 " base, "refresh" accent]
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content.as_ref(), "Add");
        assert_eq!(spans[1].content.as_ref(), " F5 ");
        assert_eq!(spans[2].content.as_ref(), "refresh");
    }
}
