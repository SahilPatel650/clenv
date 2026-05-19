use crate::env::HealthStatus;
use crate::tui::app::{AppState, HitRect, SortDir, SortField, Tab};
use crate::tui::onboarding::OnboardingField;
use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};
use std::path::Path;
use std::time::{Duration, SystemTime};

fn health_color(status: &HealthStatus) -> Color {
    match status {
        HealthStatus::Ok => Color::Green,
        HealthStatus::Warnings(_) => Color::Yellow,
        HealthStatus::Broken(_) => Color::Red,
        HealthStatus::Unknown => Color::DarkGray,
    }
}

fn format_age(t: Option<SystemTime>) -> String {
    let Some(t) = t else {
        return "unknown".to_string();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(t) else {
        return "unknown".to_string();
    };
    if elapsed < Duration::from_secs(60 * 60 * 24) {
        "today".to_string()
    } else if elapsed < Duration::from_secs(60 * 60 * 24 * 7) {
        format!("{} days ago", elapsed.as_secs() / 86400)
    } else if elapsed < Duration::from_secs(60 * 60 * 24 * 30) {
        format!("{} weeks ago", elapsed.as_secs() / (86400 * 7))
    } else {
        format!("{} months ago", elapsed.as_secs() / (86400 * 30))
    }
}

fn sort_label(field: &SortField, active: &SortField, dir: &SortDir) -> String {
    let arrow = if field == active {
        if *dir == SortDir::Desc { " ▼" } else { " ▲" }
    } else {
        ""
    };
    format!("[{}{}]", field.label(), arrow)
}

fn abbreviated_path(path: &Path, home: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(home) {
        format!("~/{}", rel.display())
    } else {
        path.to_string_lossy().to_string()
    }
}

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Length(1), // sort bar
            Constraint::Min(5),    // table
            Constraint::Length(5), // detail panel
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_tabs(frame, app, chunks[0]);
    render_sort_bar(frame, app, chunks[1]);
    render_table(frame, app, chunks[2]);
    render_detail(frame, app, chunks[3]);
    render_status_bar(frame, app, chunks[4]);

    if app.onboarding.is_some() {
        render_onboarding_overlay(frame, app, area);
    } else if app.show_tab_manager {
        render_tab_manager_overlay(frame, app, area);
    }
    if app.show_help {
        render_help_overlay(frame, area);
    }
    if app.confirm_delete {
        render_confirm_dialog(frame, app, area);
    }
}

fn render_tabs(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" clenv ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Reserve space on the right for the tab manager button "[⚙]"
    let mgr_label = "[⚙]";
    let mgr_width = 3u16;
    let tab_area_width = inner.width.saturating_sub(mgr_width + 1);

    let visible = app.visible_tabs();
    let mut spans: Vec<Span> = Vec::new();
    let mut tab_rects: Vec<HitRect> = Vec::new();
    let mut x = inner.x;

    for (i, tab) in visible.iter().enumerate() {
        let label = format!(" {} ", tab.label());
        let label_width = label.len() as u16;

        // Stop if we'd overflow into the button area
        if x + label_width > inner.x + tab_area_width {
            break;
        }

        let style = if **tab == app.active_tab {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
        };

        tab_rects.push(HitRect {
            x,
            y: inner.y,
            width: label_width,
            height: inner.height.min(1),
        });
        spans.push(Span::styled(label, style));
        x += label_width;

        if i < visible.len() - 1 {
            spans.push(Span::raw("│"));
            x += 1;
        }
    }

    app.tab_rects = tab_rects;

    // Render the tab labels
    let tabs_area = Rect {
        x: inner.x,
        y: inner.y,
        width: tab_area_width,
        height: inner.height.min(1),
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs_area);

    // Render the ⚙ manager button flush to the right
    let mgr_x = inner.x + inner.width.saturating_sub(mgr_width);
    let mgr_style = if app.show_tab_manager {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    app.tab_manager_rect = HitRect {
        x: mgr_x,
        y: inner.y,
        width: mgr_width,
        height: 1,
    };
    let mgr_area = Rect {
        x: mgr_x,
        y: inner.y,
        width: mgr_width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Span::styled(mgr_label, mgr_style)),
        mgr_area,
    );
}

fn render_sort_bar(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    let mut sort_rects: Vec<HitRect> = Vec::new();
    let mut x = area.x;

    for field in SortField::ALL {
        let label = sort_label(field, &app.sort_field, &app.sort_dir);
        // char count for display width (▼/▲ are single-width chars)
        let label_width = label.chars().count() as u16;

        let style = if field == &app.sort_field {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        sort_rects.push(HitRect {
            x,
            y: area.y,
            width: label_width,
            height: 1,
        });
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
        x += label_width + 2;
    }

    let search_display = if app.searching {
        Span::styled(
            format!("Search: {}█", app.search),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else if app.search.is_empty() {
        Span::raw("/ to search")
    } else {
        Span::styled(
            format!("Search: {}", app.search),
            Style::default().fg(Color::White),
        )
    };
    spans.push(Span::raw("  "));
    spans.push(search_display);

    app.sort_rects = sort_rects;
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_table(frame: &mut Frame, app: &mut AppState, area: Rect) {
    // account for borders (top+bottom) and header row
    let max_data_rows = area.height.saturating_sub(3) as usize;
    app.visible_rows = max_data_rows.max(1);

    let filtered = app.filtered_envs();
    let visible_start = app.scroll_offset.min(filtered.len());

    let header = Row::new(vec![
        Cell::from("  Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Path").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Size").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Health").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Version").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let mut rows: Vec<Row> = Vec::new();
    let mut row_count = 0usize;

    for (offset, env) in filtered[visible_start..].iter().enumerate() {
        if row_count >= max_data_rows {
            break;
        }
        let idx = visible_start + offset;
        let is_selected = idx == app.selected;
        let is_expanded = app.expanded_envs.contains(&env.path);

        let cursor = if is_selected { "▶" } else { " " };
        let expand_marker = if is_expanded { "▾" } else { " " };
        let size_str = format_size(env.size_bytes, BINARY);
        let version_str = env.version.as_deref().unwrap_or("—").to_string();
        let path_str = abbreviated_path(&env.path, &app.home_dir);

        let row_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        rows.push(
            Row::new(vec![
                Cell::from(format!("{cursor}{expand_marker} {}", env.name)),
                Cell::from(path_str),
                Cell::from(size_str),
                Cell::from(env.health.symbol())
                    .style(Style::default().fg(health_color(&env.health))),
                Cell::from(version_str),
            ])
            .style(row_style),
        );
        row_count += 1;

        if is_expanded && row_count < max_data_rows {
            let packages = env
                .package_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".to_string());
            let last_used = format_age(env.last_accessed);
            let cache = format_size(env.cache_size_bytes, BINARY);
            let health_msgs = env.health.messages().join(", ");
            let full_path = env.path.to_string_lossy().to_string();

            rows.push(
                Row::new(vec![
                    Cell::from(format!("  ↳ pkgs:{packages}  cache:{cache}")),
                    Cell::from(format!("  {full_path}")),
                    Cell::from(last_used),
                    Cell::from(if health_msgs.is_empty() {
                        String::new()
                    } else {
                        health_msgs
                    }),
                    Cell::from(""),
                ])
                .style(Style::default().fg(Color::Cyan)),
            );
            row_count += 1;
        }
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(40),
            Constraint::Percentage(12),
            Constraint::Percentage(8),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::TOP | Borders::BOTTOM));

    frame.render_widget(table, area);
}

fn render_detail(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL);
    let Some(env) = app.selected_env() else {
        frame.render_widget(block, area);
        return;
    };

    let block = block.title(format!(" {} ", env.name));
    let path_str = abbreviated_path(&env.path, &app.home_dir);

    let packages = env
        .package_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "—".to_string());
    let cache = format_size(env.cache_size_bytes, BINARY);
    let last_used = format_age(env.last_accessed);
    let health_msgs = env.health.messages().join(", ");
    let health_display = if health_msgs.is_empty() {
        env.health.symbol().to_string()
    } else {
        format!("{} — {}", env.health.symbol(), health_msgs)
    };
    let activation = env.activation_cmd.as_deref().unwrap_or("—");

    let text = vec![
        Line::from(vec![
            Span::styled("Path:     ", Style::default().fg(Color::Cyan)),
            Span::raw(path_str),
        ]),
        Line::from(vec![
            Span::styled("Packages: ", Style::default().fg(Color::Cyan)),
            Span::raw(packages),
            Span::raw("    "),
            Span::styled("Last used: ", Style::default().fg(Color::Cyan)),
            Span::raw(last_used),
            Span::raw("    "),
            Span::styled("Cache: ", Style::default().fg(Color::Cyan)),
            Span::raw(cache),
        ]),
        Line::from(vec![
            Span::styled("Health:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                health_display,
                Style::default().fg(health_color(&env.health)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Activate: ", Style::default().fg(Color::Cyan)),
            Span::raw(activation),
        ]),
    ];

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_status_bar(frame: &mut Frame, app: &AppState, area: Rect) {
    let msg = if app.searching {
        Span::styled(
            "[Esc] exit search  [↑↓] navigate  [Backspace] delete char",
            Style::default().fg(Color::Cyan),
        )
    } else if let Some(status) = &app.status_message {
        Span::styled(status.as_str(), Style::default().fg(Color::Green))
    } else if app.rescanning {
        Span::styled(
            "Scanning in background…",
            Style::default().fg(Color::Yellow),
        )
    } else if app.show_tab_manager {
        Span::styled(
            "[↑↓] navigate  [Space/Enter] toggle  [Esc] close tab manager",
            Style::default().fg(Color::Cyan),
        )
    } else {
        Span::raw(
            "[d] delete  [c] cache  [a] activate  [y] copy  [r] refresh  [/] search  [⚙] tabs  [?] help  [q] quit",
        )
    };
    frame.render_widget(Paragraph::new(Line::from(msg)), area);
}

fn render_onboarding_overlay(frame: &mut Frame, app: &AppState, area: Rect) {
    let Some(ob) = &app.onboarding else { return };

    // Compute popup height based on visible completions
    let comp_roots = if ob.field == OnboardingField::Roots { ob.completions.len().min(5) as u16 } else { 0 };
    let comp_ignore = if ob.field == OnboardingField::Ignore { ob.completions.len().min(5) as u16 } else { 0 };
    let inner_h = 14 + comp_roots + comp_ignore; // static lines + completions
    let height = (inner_h + 2).min(area.height.saturating_sub(2)).max(14);
    let width = 72_u16.min(area.width.saturating_sub(4)).max(40);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect { x, y, width, height };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " clenv — First-Run Setup ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let pad = "  ";
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{pad}Welcome! Configure clenv and press Enter to save."),
        Style::default().fg(Color::White),
    )));
    lines.push(Line::from(""));

    // ── Scan Roots field ─────────────────────────────────────────────────────
    let roots_active = ob.field == OnboardingField::Roots;
    lines.push(onboarding_label("Scan Roots", "(comma-separated paths)", roots_active));
    lines.push(onboarding_input(&ob.roots_input, roots_active));
    if roots_active {
        for (i, comp) in ob.completions.iter().take(5).enumerate() {
            lines.push(onboarding_completion(comp, i == ob.completion_idx));
        }
    }
    lines.push(Line::from(""));

    // ── Max Depth field ───────────────────────────────────────────────────────
    let depth_active = ob.field == OnboardingField::DepthLimit;
    lines.push(onboarding_label("Max Scan Depth", "(number of directory levels)", depth_active));
    lines.push(onboarding_input(&ob.depth_input, depth_active));
    lines.push(Line::from(""));

    // ── Ignore Paths field ────────────────────────────────────────────────────
    let ignore_active = ob.field == OnboardingField::Ignore;
    lines.push(onboarding_label("Ignore Paths", "(comma-separated, blank to skip)", ignore_active));
    let ignore_display = if ob.ignore_input.is_empty() && !ignore_active {
        "(empty)".to_string()
    } else {
        ob.ignore_input.clone()
    };
    lines.push(onboarding_input(&ignore_display, ignore_active));
    if ignore_active {
        for (i, comp) in ob.completions.iter().take(5).enumerate() {
            lines.push(onboarding_completion(comp, i == ob.completion_idx));
        }
    }
    lines.push(Line::from(""));

    // ── Hint bar ─────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled(
        format!("{pad}[Tab/↑↓] completions  [Enter] next  [Shift+Tab] prev  [Esc] skip"),
        Style::default().fg(Color::Cyan),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn onboarding_label<'a>(label: &'static str, hint: &'static str, active: bool) -> Line<'a> {
    if active {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(hint, Style::default().fg(Color::White)),
        ])
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(Color::White)),
            Span::raw(" "),
            Span::styled(hint, Style::default()),
        ])
    }
}

fn onboarding_input(value: &str, active: bool) -> Line<'static> {
    let owned = value.to_string();
    if active {
        Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(Color::Cyan)),
            Span::styled(owned, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    } else {
        let style = if value.is_empty() || value == "(empty)" {
            Style::default()
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(vec![Span::raw("    "), Span::styled(owned, style)])
    }
}

fn onboarding_completion(comp: &str, selected: bool) -> Line<'static> {
    let owned = format!("    {comp}");
    if selected {
        Line::from(Span::styled(
            owned,
            Style::default().add_modifier(Modifier::REVERSED),
        ))
    } else {
        Line::from(Span::styled(owned, Style::default()))
    }
}

fn render_tab_manager_overlay(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let overlay_width: u16 = 22;
    let overlay_height: u16 = Tab::ALL.len() as u16 + 2; // +2 for borders
    let x = area.x + area.width.saturating_sub(overlay_width + 1);
    let y = area.y + 3; // just below the tab bar
    let popup_area = Rect {
        x,
        y,
        width: overlay_width.min(area.width),
        height: overlay_height.min(area.height.saturating_sub(3)),
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Tabs ")
        .style(Style::default().bg(Color::Black));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let mut item_rects: Vec<HitRect> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();

    for (i, tab) in Tab::ALL.iter().enumerate() {
        let is_hidden = app.hidden_tabs.contains(tab);
        let checkbox = if is_hidden { "[ ] " } else { "[x] " };
        let is_cursor = i == app.tab_manager_cursor;

        let row_y = inner.y + i as u16;
        item_rects.push(HitRect {
            x: inner.x,
            y: row_y,
            width: inner.width,
            height: 1,
        });

        let style = if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else if is_hidden {
            Style::default()
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(checkbox, style),
            Span::styled(tab.label(), style),
        ]));
    }

    app.tab_manager_item_rects = item_rects;
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 80, area);
    frame.render_widget(Clear, popup_area);
    let help_text = vec![
        Line::from(""),
        Line::from("  Keybindings"),
        Line::from("  ───────────"),
        Line::from("  Tab / Shift+Tab   Cycle tabs (or click tab bar)"),
        Line::from("  ↑ / ↓  k / j      Navigate list"),
        Line::from("  PgUp / PgDn       Jump 10 rows"),
        Line::from("  Scroll wheel      Scroll list"),
        Line::from("  s                 Cycle sort field"),
        Line::from("  Click sort label  Set sort / toggle direction"),
        Line::from("  /                 Enter search mode"),
        Line::from("  Space              Expand / collapse env details"),
        Line::from("  Esc (search)      Exit search, clear query"),
        Line::from("  d                 Delete selected env"),
        Line::from("  c                 Clear cache"),
        Line::from("  a                 Print activation command"),
        Line::from("  y                 Copy activation to clipboard"),
        Line::from("  r                 Refresh scan"),
        Line::from("  ?                 Toggle this help"),
        Line::from("  q                 Quit"),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .style(Style::default().bg(Color::Black));
    frame.render_widget(Paragraph::new(help_text).block(block), popup_area);
}

fn render_confirm_dialog(frame: &mut Frame, app: &AppState, area: Rect) {
    let popup_area = centered_rect(50, 20, area);
    frame.render_widget(Clear, popup_area);
    let size_str = app
        .selected_env()
        .map(|e| format_size(e.size_bytes, BINARY))
        .unwrap_or_default();
    let text = vec![
        Line::from(""),
        Line::from(format!(
            "  Delete this environment? ({size_str} will be freed)"
        )),
        Line::from(""),
        Line::from("  [y] Yes   [n] No"),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Delete ")
        .style(Style::default().bg(Color::Black).fg(Color::Red));
    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
