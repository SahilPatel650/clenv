use crate::env::HealthStatus;
use crate::tui::app::{AppState, SortDir, SortField, Tab};
use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};
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

pub fn render(frame: &mut Frame, app: &AppState) {
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

    if app.show_help {
        render_help_overlay(frame, area);
    }
    if app.confirm_delete {
        render_confirm_dialog(frame, app, area);
    }
}

fn render_tabs(frame: &mut Frame, app: &AppState, area: Rect) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.label())).collect();
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(Block::default().borders(Borders::ALL).title(" clenv "))
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

fn render_sort_bar(frame: &mut Frame, app: &AppState, area: Rect) {
    let fields = [
        SortField::Size,
        SortField::Name,
        SortField::LastUsed,
        SortField::Health,
    ];
    let mut spans: Vec<Span> = fields
        .iter()
        .flat_map(|f| {
            let label = sort_label(f, &app.sort_field, &app.sort_dir);
            let style = if f == &app.sort_field {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            [Span::styled(label, style), Span::raw("  ")]
        })
        .collect();

    let search_display = if app.search.is_empty() {
        Span::styled("Search: _", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(
            format!("Search: {}", app.search),
            Style::default().fg(Color::White),
        )
    };
    spans.push(Span::raw("  "));
    spans.push(search_display);

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_table(frame: &mut Frame, app: &AppState, area: Rect) {
    let filtered = app.filtered_envs();
    let visible_start = app.scroll_offset.min(filtered.len());
    let visible_end = (visible_start + area.height as usize).min(filtered.len());

    let header = Row::new(vec![
        Cell::from("  Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Path").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Size").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Health").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Version").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = filtered[visible_start..visible_end]
        .iter()
        .enumerate()
        .map(|(i, env)| {
            let idx = visible_start + i;
            let is_selected = idx == app.selected;
            let cursor = if is_selected { "▶" } else { " " };
            let size_str = format_size(env.size_bytes, BINARY);
            let version_str = env.version.as_deref().unwrap_or("—").to_string();
            let path_str = {
                let home = dirs::home_dir().unwrap_or_default();
                if let Ok(rel) = env.path.strip_prefix(&home) {
                    format!("~/{}", rel.display())
                } else {
                    env.path.to_string_lossy().to_string()
                }
            };
            let row_style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format!("{cursor} {}", env.name)),
                Cell::from(path_str),
                Cell::from(size_str),
                Cell::from(env.health.symbol())
                    .style(Style::default().fg(health_color(&env.health))),
                Cell::from(version_str),
            ])
            .style(row_style)
        })
        .collect();

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
    let home = dirs::home_dir().unwrap_or_default();
    let path_str = if let Ok(rel) = env.path.strip_prefix(&home) {
        format!("~/{}", rel.display())
    } else {
        env.path.to_string_lossy().to_string()
    };

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
            Span::styled("Path:     ", Style::default().fg(Color::DarkGray)),
            Span::raw(path_str),
        ]),
        Line::from(vec![
            Span::styled("Packages: ", Style::default().fg(Color::DarkGray)),
            Span::raw(packages),
            Span::raw("    "),
            Span::styled("Last used: ", Style::default().fg(Color::DarkGray)),
            Span::raw(last_used),
            Span::raw("    "),
            Span::styled("Cache: ", Style::default().fg(Color::DarkGray)),
            Span::raw(cache),
        ]),
        Line::from(vec![
            Span::styled("Health:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                health_display,
                Style::default().fg(health_color(&env.health)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Activate: ", Style::default().fg(Color::DarkGray)),
            Span::raw(activation),
        ]),
    ];

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_status_bar(frame: &mut Frame, app: &AppState, area: Rect) {
    let msg = if let Some(status) = &app.status_message {
        Span::styled(status.as_str(), Style::default().fg(Color::Green))
    } else {
        Span::raw(
            "[d] delete  [c] clear cache  [a] activate  [y] copy  [r] refresh  [/] search  [?] help  [q] quit",
        )
    };
    frame.render_widget(Paragraph::new(Line::from(msg)), area);
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 70, area);
    frame.render_widget(Clear, popup_area);
    let help_text = vec![
        Line::from(""),
        Line::from("  Keybindings"),
        Line::from("  ───────────"),
        Line::from("  Tab / Shift+Tab   Cycle tabs"),
        Line::from("  ↑ / ↓             Navigate list"),
        Line::from("  s                 Cycle sort field"),
        Line::from("  /                 Start search"),
        Line::from("  Esc               Clear search"),
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
