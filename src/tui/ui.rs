use crate::actions;
use crate::env::HealthStatus;
use crate::tui::app::{AppState, HitRect, SortDir, SortField, Tab};
use crate::tui::onboarding::OnboardingField;
use crate::tui::theme::{default_theme, Theme};
use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};
use std::path::Path;
use std::time::{Duration, SystemTime};

fn popup_block<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            title,
            Style::default().fg(theme.popup_title).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(theme.popup_bg).fg(theme.text))
}

fn health_color(status: &HealthStatus, theme: &Theme) -> Color {
    match status {
        HealthStatus::Ok => theme.ok,
        HealthStatus::Warnings(_) => theme.warn,
        HealthStatus::Broken(_) => theme.danger,
        HealthStatus::Unknown => theme.muted,
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

pub fn render(frame: &mut Frame, app: &mut AppState, config: &crate::config::Config) {
    let theme = default_theme();
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

    render_tabs(frame, app, chunks[0], &theme);

    if app.active_tab == Tab::Shell {
        // Re-layout everything below the tab bar for the Shell tab
        let below_tabs = Rect {
            x: area.x,
            y: chunks[0].y + chunks[0].height,
            width: area.width,
            height: area.height.saturating_sub(chunks[0].height),
        };
        let detail_h: u16 = if app.shell.detail_expanded { 14 } else { 5 };
        let shell_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // sub-tab bar
                Constraint::Min(5),    // content
                Constraint::Length(detail_h),
                Constraint::Length(1), // status bar
            ])
            .split(below_tabs);
        render_shell_subtab_bar(frame, app, shell_chunks[0], &theme);
        if app.shell.page == crate::tui::app::ShellPage::Modules {
            render_shell_tab(frame, app, shell_chunks[1], &theme);
        } else {
            render_shell_fileorder(frame, app, shell_chunks[1], &theme);
        }
        render_shell_detail(frame, app, shell_chunks[2], &theme);
        render_status_bar(frame, app, shell_chunks[3], &theme);
    } else {
        render_sort_bar(frame, app, chunks[1], &theme);
        render_table(frame, app, chunks[2], &theme);
        render_detail(frame, app, chunks[3], &theme);
        render_status_bar(frame, app, chunks[4], &theme);
    }

    if app.onboarding.is_some() {
        render_onboarding_overlay(frame, app, area, &theme);
    } else if app.show_tab_manager {
        render_tab_manager_overlay(frame, app, area, &theme);
    }
    if app.show_help {
        render_help_overlay(frame, area, &theme);
    }
    if app.show_settings {
        render_settings_overlay(frame, app, config, area, &theme);
    }
    if app.confirm_delete {
        render_confirm_dialog(frame, app, area, &theme);
    }
    if app.base_deps_overlay.is_some() {
        render_base_deps_overlay(frame, app, area, &theme);
    }
    if app.shell.new_block_overlay.is_some() {
        render_new_block_overlay(frame, app, area, &theme);
    }
    if app.zshrc_change_modal.is_some() {
        render_zshrc_change_modal(frame, app, area, &theme);
    }
}

fn render_tabs(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
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
                .fg(theme.accent)
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
            .fg(theme.accent)
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

fn render_sort_bar(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
    let mut spans: Vec<Span> = Vec::new();
    let mut sort_rects: Vec<HitRect> = Vec::new();
    let mut x = area.x;

    for field in SortField::ALL {
        let label = sort_label(field, &app.sort_field, &app.sort_dir);
        // char count for display width (▼/▲ are single-width chars)
        let label_width = label.chars().count() as u16;

        let style = if field == &app.sort_field {
            Style::default()
                .fg(theme.highlight)
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
            Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD),
        )
    } else if app.search.is_empty() {
        Span::raw("/ to search")
    } else {
        Span::styled(
            format!("Search: {}", app.search),
            Style::default().fg(theme.text),
        )
    };
    spans.push(Span::raw("  "));
    spans.push(search_display);

    app.sort_rects = sort_rects;
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_table(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
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
    .style(Style::default().fg(theme.accent));

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
                    .style(Style::default().fg(health_color(&env.health, theme))),
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
                .style(Style::default().fg(theme.highlight)),
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

fn render_detail(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
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
            Span::styled("Path:     ", Style::default().fg(theme.highlight)),
            Span::raw(path_str),
        ]),
        Line::from(vec![
            Span::styled("Packages: ", Style::default().fg(theme.highlight)),
            Span::raw(packages),
            Span::raw("    "),
            Span::styled("Last used: ", Style::default().fg(theme.highlight)),
            Span::raw(last_used),
            Span::raw("    "),
            Span::styled("Cache: ", Style::default().fg(theme.highlight)),
            Span::raw(cache),
        ]),
        Line::from(vec![
            Span::styled("Health:   ", Style::default().fg(theme.highlight)),
            Span::styled(
                health_display,
                Style::default().fg(health_color(&env.health, theme)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Activate: ", Style::default().fg(theme.highlight)),
            Span::raw(activation),
        ]),
    ];

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_status_bar(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    let on_shell = app.active_tab == Tab::Shell;
    let msg = if app.searching {
        Span::styled(
            "[Esc] exit search  [↑↓] navigate  [Backspace] delete char",
            Style::default().fg(theme.highlight),
        )
    } else if let Some(status) = &app.status_message {
        if on_shell {
            // Render message + dismiss hint as a line with two spans
            let line = Line::from(vec![
                Span::styled(status.as_str(), Style::default().fg(theme.ok)),
                Span::styled("  [Esc] dismiss", Style::default().fg(theme.muted)),
            ]);
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
        Span::styled(status.as_str(), Style::default().fg(theme.ok))
    } else if app.rescanning {
        Span::styled(
            "Scanning in background…",
            Style::default().fg(theme.accent),
        )
    } else if app.show_tab_manager {
        Span::styled(
            "[↑↓] navigate  [Space/Enter] toggle  [Esc] close tab manager",
            Style::default().fg(theme.highlight),
        )
    } else if app.active_tab == Tab::Shell {
        if app.shell.page == crate::tui::app::ShellPage::FileOrder {
            if app.shell.moving_block.is_some() {
                Span::styled(
                    "MOVING — [↑↓] reposition  [Enter] drop  [Esc] cancel",
                    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("[↑↓] navigate  [Enter] grab  [l] label  [◀▶] switch page  [?] help")
            }
        } else {
            Span::raw(
                "[Enter/i] install  [Space] expand  [d] disable  [n] new block  [c] AI context  [r] sync  [?] help  [q] quit",
            )
        }
    } else {
        Span::raw(
            "[d] delete  [c] cache  [a] activate  [y] copy  [r] refresh  [/] search  [2] shell  [?] help  [q] quit",
        )
    };
    frame.render_widget(Paragraph::new(Line::from(msg)), area);
}

fn render_shell_subtab_bar(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    use crate::tui::app::ShellPage;
    let modules_style = if app.shell.page == ShellPage::Modules {
        Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(theme.muted)
    };
    let fileorder_style = if app.shell.page == ShellPage::FileOrder {
        Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(theme.muted)
    };
    let spans = vec![
        Span::styled(" Modules ", modules_style),
        Span::styled("│", Style::default().fg(theme.muted)),
        Span::styled(" File Order ", fileorder_style),
        Span::styled("  ◀▶ switch", Style::default().fg(theme.muted)),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_shell_fileorder(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
    use crate::modules::zshrc::{parse_segments, SegmentKind};

    let zshrc_path = app.home_dir.join(".zshrc");
    let segments = parse_segments(&zshrc_path);

    let block = Block::default().borders(Borders::TOP | Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 { return; }

    let total = segments.len();
    let cursor = app.shell.fileorder_cursor.min(total.saturating_sub(1));
    let moving = app.shell.moving_block;

    // Clamp scroll
    if cursor < app.shell.scroll_offset {
        app.shell.scroll_offset = cursor;
    }
    if cursor >= app.shell.scroll_offset + inner.height as usize {
        app.shell.scroll_offset = cursor + 1 - inner.height as usize;
    }
    let scroll = app.shell.scroll_offset;

    let mut row_y = inner.y;

    let drop_before = if moving.is_some() { Some(cursor) } else { None };

    for (seg_idx, seg) in segments.iter().enumerate() {
        if row_y >= inner.y + inner.height { break; }

        // Drop zone above this segment when in move mode
        if let Some(target) = drop_before {
            if seg_idx == target && seg_idx != moving.unwrap_or(usize::MAX) {
                if seg_idx >= scroll {
                    let dz_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };
                    frame.render_widget(
                        Paragraph::new(Span::styled(
                            "  ── drop here ──",
                            Style::default().fg(theme.accent),
                        )),
                        dz_area,
                    );
                    row_y += 1;
                }
            }
        }

        if seg_idx < scroll { continue; }
        if row_y >= inner.y + inner.height { break; }

        let is_cursor = seg_idx == cursor;
        let is_moving = moving == Some(seg_idx);

        let (indicator, name, extra) = match &seg.kind {
            SegmentKind::Clenv(name) => {
                let status = app.shell.entries.iter()
                    .find(|e| &e.definition.name == name)
                    .map(|e| e.status.label())
                    .unwrap_or("custom");
                ("✓", name.as_str(), status.to_string())
            }
            SegmentKind::Unmanaged => {
                let lines = seg.content.lines().count();
                ("~", "unmanaged", format!("{lines} lines"))
            }
        };

        let style = if is_moving {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(theme.text)
        };

        let prefix = if is_moving { "▶▶" } else { "  " };
        let display = format!("{prefix} #{seg_idx:<2} [{indicator}] {name:<22} {extra}");

        let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };
        frame.render_widget(Paragraph::new(Span::styled(display, style)), row_area);
        row_y += 1;
    }

    // Drop zone at end (after all segments)
    if let Some(target) = drop_before {
        if target == total && row_y < inner.y + inner.height {
            let dz_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };
            frame.render_widget(
                Paragraph::new(Span::styled("  ── drop here (end) ──", Style::default().fg(theme.accent))),
                dz_area,
            );
        }
    }
}

fn render_shell_tab(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
    use crate::modules::ModuleStatus;

    const CATEGORY_ORDER: &[&str] = &[
        "package-managers",
        "shell-frameworks",
        "shell-themes",
        "zsh-plugins",
        "productivity",
        "aliases",
    ];

    fn category_rank(cat: &str) -> usize {
        CATEGORY_ORDER.iter().position(|c| *c == cat).unwrap_or(CATEGORY_ORDER.len())
    }

    // nav_index → flat_items index; flat_items includes headers.
    // nav_index 0..unmanaged.len() = unmanaged blocks
    // nav_index unmanaged.len().. = entries
    #[derive(Clone)]
    enum ListItem {
        Header(String),
        Unmanaged(usize),  // index into shell.unmanaged
        Module(usize),     // index into shell.entries
    }

    let unmanaged_count = app.shell.unmanaged.len();
    let nav_count = app.shell_nav_count();

    // Build the flat display list
    let mut flat_items: Vec<ListItem> = Vec::new();

    // UNMANAGED section (only if there are any)
    if unmanaged_count > 0 {
        flat_items.push(ListItem::Header("unmanaged".to_string()));
        for i in 0..unmanaged_count {
            flat_items.push(ListItem::Unmanaged(i));
        }
    }

    // Managed modules by category
    let mut categories: Vec<String> = {
        let mut cats: Vec<String> = app.shell.entries
            .iter()
            .map(|e| e.definition.category.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        cats.sort_by_key(|c| (category_rank(c.as_str()), c.clone()));
        cats
    };
    categories.dedup();

    for cat in &categories {
        flat_items.push(ListItem::Header(cat.clone()));
        for (idx, entry) in app.shell.entries.iter().enumerate() {
            if &entry.definition.category == cat {
                flat_items.push(ListItem::Module(idx));
            }
        }
    }

    let block = Block::default().borders(Borders::TOP | Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let max_visible_rows = inner.height as usize;
    let total_flat = flat_items.len();

    // Find flat index of currently selected nav item
    let nav_cursor = app.shell.cursor;
    let flat_cursor = flat_items.iter().position(|item| match item {
        ListItem::Unmanaged(i) => *i == nav_cursor,
        ListItem::Module(i) => unmanaged_count + *i == nav_cursor,
        ListItem::Header(_) => false,
    }).unwrap_or(0);

    // Clamp scroll
    if flat_cursor < app.shell.scroll_offset {
        app.shell.scroll_offset = flat_cursor;
    }
    if flat_cursor >= app.shell.scroll_offset + max_visible_rows {
        app.shell.scroll_offset = flat_cursor + 1 - max_visible_rows;
    }
    let scroll = app.shell.scroll_offset.min(total_flat.saturating_sub(1));

    // item_rects maps nav_index → click rect
    let mut item_rects: Vec<HitRect> = vec![HitRect::default(); nav_count.max(1)];

    let mut row_y = inner.y;
    for flat_idx in scroll..(scroll + max_visible_rows).min(total_flat) {
        if row_y >= inner.y + inner.height {
            break;
        }
        let item = &flat_items[flat_idx];
        match item {
            ListItem::Header(cat) => {
                let label = if cat == "unmanaged" {
                    "UNMANAGED  (user-written content between clenv blocks)".to_string()
                } else {
                    cat.to_uppercase().replace('-', " ")
                };
                let line = Line::from(Span::styled(
                    format!(" {label}"),
                    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                ));
                let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };
                frame.render_widget(Paragraph::new(line), row_area);
                row_y += 1;
            }
            ListItem::Unmanaged(idx) => {
                let block_entry = &app.shell.unmanaged[*idx];
                let nav_idx = *idx;
                let is_selected = nav_idx == nav_cursor;
                let label = block_entry.label();
                let line_count = block_entry.line_count;

                let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };

                let spans: Vec<Span> = if is_selected {
                    vec![Span::styled(
                        format!("  [~] {label:<40}  {line_count} lines"),
                        Style::default().add_modifier(Modifier::REVERSED),
                    )]
                } else {
                    vec![
                        Span::styled("  [", Style::default().fg(theme.muted)),
                        Span::styled("~", Style::default().fg(theme.warn)),
                        Span::styled("] ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{label:<40}"), Style::default().fg(theme.muted)),
                        Span::styled(format!("  {line_count} lines"), Style::default().fg(theme.muted)),
                    ]
                };
                frame.render_widget(Paragraph::new(Line::from(spans)), row_area);

                item_rects[nav_idx] = HitRect {
                    x: inner.x, y: row_y, width: inner.width, height: 1,
                };
                row_y += 1;
            }
            ListItem::Module(idx) => {
                let entry = &app.shell.entries[*idx];
                let nav_idx = unmanaged_count + *idx;
                let name = &entry.definition.name;
                let desc = &entry.definition.description;
                let is_selected = nav_idx == nav_cursor;
                let dep_missing = !entry.missing_deps.is_empty();
                let unavailable = !entry.can_install
                    && matches!(entry.status, ModuleStatus::NotInstalled);

                let (indicator, status_label, status_color) = if dep_missing {
                    ("⊘", "dep missing", theme.muted)
                } else if unavailable {
                    ("×", "unavailable", theme.muted)
                } else {
                    match &entry.status {
                        ModuleStatus::ManagedActive => {
                            if entry.block_diff.is_some() {
                                ("✓", "active  ±", theme.warn)
                            } else {
                                ("✓", "active", theme.ok)
                            }
                        }
                        ModuleStatus::ManagedInactive => ("○", "inactive", theme.muted),
                        ModuleStatus::NotInstalled => ("↓", "not installed", theme.highlight),
                        ModuleStatus::InstalledUnmanaged => ("~", "unmanaged", theme.warn),
                    }
                };

                let base_fg = if dep_missing || unavailable { theme.muted } else { theme.text };

                let name_w = 18usize;
                let name_str = if name.len() > name_w { &name[..name_w] } else { name };
                let name_padded = format!("{name_str:<name_w$}");
                let desc_max = inner.width.saturating_sub(38) as usize;
                let desc_str = if desc.len() > desc_max {
                    desc[..desc_max].to_string()
                } else {
                    format!("{desc:<desc_max$}")
                };
                // Show expand marker if detail panel is expanded for this item
                let expand = if is_selected && app.shell.detail_expanded { "▾" } else { "▶" };

                let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };

                let spans: Vec<Span> = if is_selected {
                    vec![Span::styled(
                        format!("  [{}] {name_padded}  {desc_str}  {status_label}  {expand}",
                            indicator),
                        Style::default().add_modifier(Modifier::REVERSED),
                    )]
                } else {
                    vec![
                        Span::styled("  [", Style::default().fg(base_fg)),
                        Span::styled(indicator, Style::default().fg(status_color)),
                        Span::styled("] ", Style::default().fg(base_fg)),
                        Span::styled(name_padded, Style::default().fg(base_fg)),
                        Span::raw("  "),
                        Span::styled(desc_str, Style::default().fg(base_fg)),
                        Span::raw("  "),
                        Span::styled(status_label, Style::default().fg(status_color)),
                        Span::raw("  "),
                        Span::styled("▶", Style::default().fg(theme.muted)),
                    ]
                };

                frame.render_widget(Paragraph::new(Line::from(spans)), row_area);

                item_rects[nav_idx] = HitRect {
                    x: inner.x, y: row_y, width: inner.width, height: 1,
                };
                row_y += 1;
            }
        }
    }

    app.shell.item_rects = item_rects;
}

fn render_shell_detail(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    // Show unmanaged block detail if cursor is in the unmanaged section
    if let Some(ub) = app.selected_unmanaged() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" unmanaged block ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Lines: ", Style::default().fg(theme.highlight)),
                Span::raw(ub.line_count.to_string()),
                Span::raw("   "),
                Span::styled("[Space] show/hide content", Style::default().fg(theme.muted)),
            ]),
        ];
        if app.shell.detail_expanded {
            lines.push(Line::from(""));
            let avail = (inner.height as usize).saturating_sub(2);
            for content_line in ub.content.lines().take(avail) {
                lines.push(Line::from(Span::styled(
                    format!("  {content_line}"),
                    Style::default().fg(theme.muted),
                )));
            }
            if ub.content.lines().count() > avail {
                lines.push(Line::from(Span::styled(
                    "  ...",
                    Style::default().fg(theme.muted),
                )));
            }
        }
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let block = Block::default().borders(Borders::ALL);
    let Some(entry) = app.selected_module() else {
        frame.render_widget(block, area);
        return;
    };

    let def = &entry.definition;
    let title = if app.shell.detail_expanded {
        format!(" {} — current content ", def.name)
    } else if entry.block_diff.is_some() {
        format!(" {} ⚠ modified ", def.name)
    } else {
        format!(" {} ", def.name)
    };
    let title_style = if entry.block_diff.is_some() && !app.shell.detail_expanded {
        Style::default().fg(theme.warn).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    let block = block.title(Span::styled(title, title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let status_label = entry.status.label();
    let startup_ms = def.zshrc.startup_ms_estimate;
    let depends = if def.depends_on.is_empty() { "none".to_string() } else { def.depends_on.join(", ") };

    let mut text = vec![
        Line::from(vec![
            Span::styled("Description:  ", Style::default().fg(theme.highlight)),
            Span::raw(def.description.clone()),
        ]),
        Line::from(vec![
            Span::styled("Status:       ", Style::default().fg(theme.highlight)),
            Span::raw(status_label),
            Span::raw("    "),
            Span::styled("Startup: ", Style::default().fg(theme.highlight)),
            if startup_ms > 0 { Span::raw(format!("~{startup_ms}ms")) }
            else { Span::styled("minimal", Style::default().fg(theme.muted)) },
        ]),
        Line::from(vec![
            Span::styled("Depends on:   ", Style::default().fg(theme.highlight)),
            if entry.missing_deps.is_empty() {
                Span::raw(depends)
            } else {
                Span::styled(
                    format!("{depends}  ⚠ missing: {}", entry.missing_deps.join(", ")),
                    Style::default().fg(theme.warn),
                )
            },
        ]),
    ];

    if app.shell.detail_expanded {
        // Show raw block content currently in .zshrc
        let zshrc_path = dirs::home_dir().unwrap_or_default().join(".zshrc");
        if let Some(content) = crate::modules::zshrc::read_block(&zshrc_path, &def.name) {
            text.push(Line::from(vec![
                Span::styled("Current content:", Style::default().fg(theme.highlight)),
            ]));
            let avail = inner.height.saturating_sub(text.len() as u16 + 1) as usize;
            for line in content.lines().take(avail) {
                text.push(Line::from(Span::styled(
                    format!("  {line}"),
                    Style::default().fg(theme.muted),
                )));
            }
        } else {
            text.push(Line::from(Span::styled(
                "  (block not present in .zshrc)",
                Style::default().fg(theme.muted),
            )));
        }
    } else {
        let platform_note = if entry.can_install { "yes" }
            else if matches!(entry.status, crate::modules::ModuleStatus::NotInstalled) { "no installer for this OS" }
            else { "—" };
        text.push(Line::from(vec![
            Span::styled("Installable:  ", Style::default().fg(theme.highlight)),
            Span::raw(platform_note),
        ]));
        text.push(Line::from(vec![
            Span::styled("zshrc order:  ", Style::default().fg(theme.highlight)),
            Span::raw(def.zshrc.order.to_string()),
        ]));
        if entry.block_diff.is_some() {
            text.push(Line::from(vec![
                Span::styled("Modified:     ", Style::default().fg(theme.highlight)),
                Span::styled(
                    "yes — press [Space] to view current content",
                    Style::default().fg(theme.warn),
                ),
            ]));
        }
        if let Some(sync_time) = app.shell.private_repo_last_sync {
            let label = format_age(Some(sync_time));
            text.push(Line::from(vec![
                Span::styled("Private repo: ", Style::default().fg(theme.highlight)),
                Span::raw(format!("synced {label}")),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(text), inner);
}

fn render_onboarding_overlay(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
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

    let block = popup_block(" clenv — First-Run Setup ", theme);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let pad = "  ";
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("{pad}Welcome! Configure clenv and press Enter to save."),
        Style::default().fg(theme.text),
    )));
    lines.push(Line::from(""));

    // ── Scan Roots field ─────────────────────────────────────────────────────
    let roots_active = ob.field == OnboardingField::Roots;
    lines.push(onboarding_label("Scan Roots", "(comma-separated paths)", roots_active, theme));
    lines.push(onboarding_input(&ob.roots_input, roots_active, theme));
    if roots_active {
        for (i, comp) in ob.completions.iter().take(5).enumerate() {
            lines.push(onboarding_completion(comp, i == ob.completion_idx));
        }
    }
    lines.push(Line::from(""));

    // ── Max Depth field ───────────────────────────────────────────────────────
    let depth_active = ob.field == OnboardingField::DepthLimit;
    lines.push(onboarding_label("Max Scan Depth", "(number of directory levels)", depth_active, theme));
    lines.push(onboarding_input(&ob.depth_input, depth_active, theme));
    lines.push(Line::from(""));

    // ── Ignore Paths field ────────────────────────────────────────────────────
    let ignore_active = ob.field == OnboardingField::Ignore;
    lines.push(onboarding_label("Ignore Paths", "(comma-separated, blank to skip)", ignore_active, theme));
    let ignore_display = if ob.ignore_input.is_empty() && !ignore_active {
        "(empty)".to_string()
    } else {
        ob.ignore_input.clone()
    };
    lines.push(onboarding_input(&ignore_display, ignore_active, theme));
    if ignore_active {
        for (i, comp) in ob.completions.iter().take(5).enumerate() {
            lines.push(onboarding_completion(comp, i == ob.completion_idx));
        }
    }
    lines.push(Line::from(""));

    // ── Hint bar ─────────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled(
        format!("{pad}[Tab/↑↓] completions  [Enter] next  [Shift+Tab] prev  [Esc] skip"),
        Style::default().fg(theme.highlight),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn onboarding_label<'a>(label: &'static str, hint: &'static str, active: bool, theme: &Theme) -> Line<'a> {
    if active {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(hint, Style::default().fg(theme.text)),
        ])
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(theme.text)),
            Span::raw(" "),
            Span::styled(hint, Style::default()),
        ])
    }
}

fn onboarding_input(value: &str, active: bool, theme: &Theme) -> Line<'static> {
    let owned = value.to_string();
    if active {
        Line::from(vec![
            Span::styled("  ▶ ", Style::default().fg(theme.highlight)),
            Span::styled(owned, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(theme.highlight)),
        ])
    } else {
        let style = if value.is_empty() || value == "(empty)" {
            Style::default()
        } else {
            Style::default().fg(theme.text)
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

fn render_tab_manager_overlay(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
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

    let block = popup_block(" Tabs ", theme);
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
            Style::default().fg(theme.text)
        };

        lines.push(Line::from(vec![
            Span::styled(checkbox, style),
            Span::styled(tab.label(), style),
        ]));
    }

    app.tab_manager_item_rects = item_rects;
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_help_overlay(frame: &mut Frame, area: Rect, theme: &Theme) {
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
        Line::from("  .                 Open settings"),
        Line::from("  q                 Quit"),
    ];
    let block = popup_block(" Help ", theme);
    frame.render_widget(Paragraph::new(help_text).block(block), popup_area);
}

pub fn render_settings_overlay(frame: &mut Frame, app: &AppState, config: &crate::config::Config, area: Rect, theme: &Theme) {
    use crate::tui::app::SettingsTab;

    let width = (area.width * 3 / 4).max(60).min(area.width.saturating_sub(4));
    let height = (area.height * 2 / 3).max(16).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect { x, y, width, height };

    frame.render_widget(Clear, popup_area);

    let block = popup_block(" Settings ", theme);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.width < 20 || inner.height < 4 { return; }

    let tab_strip_w: u16 = 10;
    let tab_area  = Rect { x: inner.x, y: inner.y, width: tab_strip_w, height: inner.height };
    let content_area = Rect {
        x: inner.x + tab_strip_w + 1,
        y: inner.y,
        width: inner.width.saturating_sub(tab_strip_w + 1),
        height: inner.height.saturating_sub(1),
    };
    let hint_area = Rect {
        x: inner.x, y: inner.y + inner.height.saturating_sub(1),
        width: inner.width, height: 1,
    };

    for row in 0..inner.height {
        let cell_area = Rect { x: inner.x + tab_strip_w, y: inner.y + row, width: 1, height: 1 };
        frame.render_widget(
            Paragraph::new(Span::styled("│", Style::default().fg(theme.muted))),
            cell_area,
        );
    }

    for (i, &tab) in SettingsTab::ALL.iter().enumerate() {
        let is_active = tab == app.settings_state.tab;
        let style = if is_active {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        let prefix = if is_active { "▌" } else { " " };
        let label = format!("{}{}", prefix, tab.label());
        let row_area = Rect { x: tab_area.x, y: tab_area.y + i as u16, width: tab_area.width, height: 1 };
        frame.render_widget(Paragraph::new(Span::styled(label, style)), row_area);
    }

    let st = &app.settings_state;
    match st.tab {
        SettingsTab::Shell => render_settings_shell(frame, app, config, content_area, theme),
        SettingsTab::Scan  => render_settings_scan(frame, app, config, content_area, theme),
        SettingsTab::Ui    => render_settings_ui(frame, app, config, content_area, theme),
    }

    let hint = if st.editing.is_some() {
        "[Enter] save   [Esc] cancel edit"
    } else {
        "[←→] tab   [↑↓] navigate   [Enter/Space] edit/toggle   [Esc] close & save"
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::default().fg(theme.muted))),
        hint_area,
    );
}

fn settings_row(
    frame: &mut Frame,
    label: &str,
    value: &str,
    is_selected: bool,
    is_editing: bool,
    y: u16,
    area: Rect,
    theme: &Theme,
) {
    let label_w = 24usize;
    let label_str = format!("{label:<label_w$}");
    let value_str = if is_editing { format!("{value}█") } else { value.to_string() };
    let cursor = if is_selected { "▶" } else { " " };
    let (label_style, value_style) = if is_selected {
        (
            Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )
    } else {
        (Style::default().fg(theme.muted), Style::default().fg(theme.text))
    };
    let spans = vec![
        Span::raw(format!("{cursor} ")),
        Span::styled(label_str, label_style),
        Span::styled(value_str, value_style),
    ];
    let row_area = Rect { x: area.x, y, width: area.width, height: 1 };
    frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
}

fn settings_toggle_row(
    frame: &mut Frame,
    label: &str,
    value: bool,
    is_selected: bool,
    y: u16,
    area: Rect,
    theme: &Theme,
) {
    let label_w = 24usize;
    let label_str = format!("{label:<label_w$}");
    let (toggle_str, toggle_color) = if value { ("[ON ]", theme.ok) } else { ("[OFF]", theme.muted) };
    let cursor = if is_selected { "▶" } else { " " };
    let label_style = if is_selected {
        Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    let spans = vec![
        Span::raw(format!("{cursor} ")),
        Span::styled(label_str, label_style),
        Span::styled(toggle_str, Style::default().fg(toggle_color).add_modifier(Modifier::BOLD)),
    ];
    let row_area = Rect { x: area.x, y, width: area.width, height: 1 };
    frame.render_widget(Paragraph::new(Line::from(spans)), row_area);
}

fn render_settings_shell(
    frame: &mut Frame,
    app: &AppState,
    config: &crate::config::Config,
    area: Rect,
    theme: &Theme,
) {
    let st = &app.settings_state;
    let rows: &[(&str, usize)] = &[
        ("zshrc path",             0),
        ("Private dotfiles repo",  1),
        ("Agent context repo",     2),
        ("Auto-detect on install", 3),
        ("Watch zshrc changes",    4),
    ];
    for &(label, row_idx) in rows {
        if row_idx as u16 >= area.height { break; }
        let is_sel = st.cursor == row_idx;
        let is_edit = st.editing == Some(row_idx);
        let y = area.y + row_idx as u16;
        match row_idx {
            0 => {
                let val = config.modules.zshrc_path.as_deref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "~/.zshrc (default)".to_string());
                let display = if is_edit { st.input_buf.clone() } else { val };
                settings_row(frame, label, &display, is_sel, is_edit, y, area, theme);
            }
            1 => {
                let val = config.modules.private_dotfiles_repo.clone()
                    .unwrap_or_else(|| "not set".to_string());
                let display = if is_edit { st.input_buf.clone() } else { val };
                settings_row(frame, label, &display, is_sel, is_edit, y, area, theme);
            }
            2 => {
                let val = config.modules.agent_context_repo.clone()
                    .unwrap_or_else(|| "not set".to_string());
                let display = if is_edit { st.input_buf.clone() } else { val };
                settings_row(frame, label, &display, is_sel, is_edit, y, area, theme);
            }
            3 => {
                settings_toggle_row(frame, label, config.ui.auto_detect_after_install, is_sel, y, area, theme);
            }
            4 => {
                settings_toggle_row(frame, label, config.modules.watch_zshrc, is_sel, y, area, theme);
            }
            _ => {}
        }
    }
}

fn render_settings_scan(
    frame: &mut Frame,
    app: &AppState,
    config: &crate::config::Config,
    area: Rect,
    theme: &Theme,
) {
    let st = &app.settings_state;
    let depth_val = config.scan.depth_limit.to_string();
    let is_sel = st.cursor == 0;
    let is_edit = st.editing == Some(0);
    let display = if is_edit { st.input_buf.clone() } else { depth_val };
    settings_row(frame, "Depth limit", &display, is_sel, is_edit, area.y, area, theme);

    let mut y = area.y + 1;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  Scan roots:", Style::default().fg(theme.highlight)))),
        Rect { x: area.x, y, width: area.width, height: 1 },
    );
    y += 1;

    for (i, root) in config.scan.roots.iter().enumerate() {
        let row_idx = 1 + i;
        if y >= area.y + area.height { break; }
        let is_sel = st.cursor == row_idx;
        let is_edit = st.editing == Some(row_idx);
        let val = root.to_string_lossy().to_string();
        let display = if is_edit { st.input_buf.clone() } else { val };
        settings_row(frame, "", &display, is_sel, is_edit, y, area, theme);
        y += 1;
    }

    let add_idx = 1 + config.scan.roots.len();
    if y < area.y + area.height {
        let is_sel = st.cursor == add_idx;
        let style = if is_sel {
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        let prefix = if is_sel { "▶ " } else { "  " };
        frame.render_widget(
            Paragraph::new(Span::styled(format!("{prefix}[+ add root]"), style)),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );
    }
}

fn render_settings_ui(
    frame: &mut Frame,
    app: &AppState,
    config: &crate::config::Config,
    area: Rect,
    theme: &Theme,
) {
    let st = &app.settings_state;
    let rows: &[(&str, usize)] = &[
        ("Default tab",      0),
        ("Default sort",     1),
        ("Default sort dir", 2),
    ];
    for &(label, row_idx) in rows {
        if row_idx as u16 >= area.height { break; }
        let is_sel = st.cursor == row_idx;
        let is_edit = st.editing == Some(row_idx);
        let y = area.y + row_idx as u16;
        let val = match row_idx {
            0 => config.ui.default_tab.clone(),
            1 => config.ui.default_sort.clone(),
            2 => config.ui.default_sort_dir.clone(),
            _ => String::new(),
        };
        let display = if is_edit { st.input_buf.clone() } else { val };
        settings_row(frame, label, &display, is_sel, is_edit, y, area, theme);
    }
}

fn render_confirm_dialog(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    let popup_area = centered_rect(60, 30, area);
    frame.render_widget(Clear, popup_area);

    let (size_str, cmd) = app
        .selected_env()
        .map(|e| (format_size(e.size_bytes, BINARY), actions::delete_preview(e)))
        .unwrap_or_default();

    let streams = app
        .selected_env()
        .map(|e| actions::delete_streams_output(e))
        .unwrap_or(false);

    let output_note = if streams {
        "  Command output will be shown in the terminal."
    } else {
        "  Directory will be removed immediately."
    };

    let text = vec![
        Line::from(""),
        Line::from(format!("  Delete this environment? ({size_str} will be freed)")),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Command: "),
            Span::styled(cmd, Style::default().fg(theme.accent)),
        ]),
        Line::from(""),
        Line::from(Span::styled(output_note, Style::default().fg(theme.muted))),
        Line::from(""),
        Line::from("  [y] Yes   [n / Esc] No"),
    ];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Confirm Delete ")
        .style(Style::default().bg(theme.popup_bg).fg(theme.danger));
    frame.render_widget(Paragraph::new(text).block(block), popup_area);
}

fn render_base_deps_overlay(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    let Some(overlay) = &app.base_deps_overlay else { return };

    let popup_area = centered_rect(60, 50, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            " Missing Base Requirements ",
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(theme.warn))
        .style(Style::default().bg(theme.popup_bg));

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  The following tools are required by install scripts:",
            Style::default().fg(theme.text),
        )),
        Line::from(""),
    ];

    for dep in &overlay.missing {
        lines.push(Line::from(vec![
            Span::styled("    • ", Style::default().fg(theme.warn)),
            Span::styled(dep.as_str(), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Install them first for best results.",
        Style::default().fg(theme.muted),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [y] ", Style::default().fg(theme.ok).add_modifier(Modifier::BOLD)),
        Span::styled("proceed anyway   ", Style::default().fg(theme.text)),
        Span::styled("[any other key] ", Style::default().fg(theme.muted)),
        Span::styled("cancel", Style::default().fg(theme.text)),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_new_block_overlay(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    use crate::tui::app::NewBlockFocus;
    let Some(overlay) = &app.shell.new_block_overlay else { return };

    let popup_area = centered_rect(70, 75, area);
    frame.render_widget(Clear, popup_area);
    let block = popup_block(" New Shell Block ", theme);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 8 {
        return;
    }

    // Fixed rows for name + description fields; remaining = position list
    let field_h = 5u16;
    let pos_h = inner.height.saturating_sub(field_h + 1);
    let field_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(field_h),
            Constraint::Length(1),
            Constraint::Length(pos_h),
        ])
        .split(inner);

    // ── Name + Description fields ─────────────────────────────────────────────
    let name_active = overlay.focus == NewBlockFocus::Name;
    let desc_active = overlay.focus == NewBlockFocus::Description;

    let name_style = if name_active { Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD) }
        else { Style::default().fg(theme.muted) };
    let desc_style = if desc_active { Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD) }
        else { Style::default().fg(theme.muted) };

    let cursor = |active: bool| if active { "█" } else { "" };

    let field_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Name:        ", name_style),
            Span::styled(format!("{}{}", overlay.name, cursor(name_active)), Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Description: ", desc_style),
            Span::styled(format!("{}{}", overlay.description, cursor(desc_active)), Style::default().fg(theme.text)),
        ]),
        Line::from(""),
    ];
    frame.render_widget(Paragraph::new(field_lines), field_chunks[0]);

    // ── Position label ────────────────────────────────────────────────────────
    let pos_active = overlay.focus == NewBlockFocus::Position;
    let pos_label_style = if pos_active { Style::default().fg(theme.highlight).add_modifier(Modifier::BOLD) }
        else { Style::default().fg(theme.muted) };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("  Insert position (↑↓ to move):", pos_label_style))),
        field_chunks[1],
    );

    // ── Position list ─────────────────────────────────────────────────────────
    let pos_inner = field_chunks[2];
    let max_rows = pos_inner.height as usize;
    let total = overlay.position_items.len();
    let scroll_start = overlay.position_cursor.saturating_sub(max_rows / 2)
        .min(total.saturating_sub(max_rows));
    let visible = &overlay.position_items[scroll_start..(scroll_start + max_rows).min(total)];

    let mut pos_lines: Vec<Line> = Vec::new();
    for (i, item) in visible.iter().enumerate() {
        let abs_idx = scroll_start + i;
        let is_cursor = abs_idx == overlay.position_cursor;
        let style = if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else if pos_active {
            Style::default().fg(theme.text)
        } else {
            Style::default().fg(theme.muted)
        };
        let marker = if is_cursor { "▶" } else { " " };
        pos_lines.push(Line::from(Span::styled(
            format!("  {marker} {}", item.label),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(pos_lines), pos_inner);

    // ── Hint bar at bottom of popup ───────────────────────────────────────────
    // (render inside inner area's last row if space allows)
    if inner.height > 0 {
        let hint_y = inner.y + inner.height.saturating_sub(1);
        let hint_area = Rect { x: inner.x, y: hint_y, width: inner.width, height: 1 };
        frame.render_widget(
            Paragraph::new(Span::styled(
                "  [Tab] next field  [Enter] create  [Esc] cancel",
                Style::default().fg(theme.muted),
            )),
            hint_area,
        );
    }
}

pub fn render_zshrc_change_modal(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    let Some(modal) = &app.zshrc_change_modal else { return };

    let width = (area.width * 4 / 5).min(100).max(60);
    let height = 14u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect { x, y, width, height };

    frame.render_widget(Clear, popup_area);

    let title = modal.block.name.as_deref()
        .map(|n| format!(" \u{26a0} ~/.zshrc changed \u{2014} {n} "))
        .unwrap_or_else(|| " \u{26a0} ~/.zshrc changed \u{2014} unmanaged block ".to_string());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(theme.warn).add_modifier(Modifier::BOLD)))
        .style(Style::default().bg(theme.popup_bg).fg(theme.text));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    if inner.height < 4 { return; }

    let header = Line::from(Span::styled(
        "New content detected. Choose which version to keep:",
        Style::default().fg(theme.text),
    ));
    frame.render_widget(Paragraph::new(header), Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 });

    let col_w = (inner.width.saturating_sub(2)) / 3;

    let columns: [(u8, &str, Option<&str>); 3] = [
        (1, "[1] Install script", Some(modal.block.new_content.as_str())),
        (2, "[2] clenv canonical", modal.block.canonical_content.as_deref()),
        (3, "[3] My custom config", modal.block.custom_content.as_deref()),
    ];

    for (i, (choice, label, content_opt)) in columns.iter().enumerate() {
        let col_x = inner.x + (col_w + 1) * i as u16;
        let is_selected = modal.selected == *choice;
        let border_color = if is_selected { theme.accent } else { theme.muted };

        let col_area = Rect { x: col_x, y: inner.y + 1, width: col_w, height: inner.height.saturating_sub(2) };
        let col_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(*label, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
            .style(Style::default().bg(theme.popup_bg));
        let col_inner = col_block.inner(col_area);
        frame.render_widget(col_block, col_area);

        if let Some(content) = content_opt {
            if !content.is_empty() {
                let max_w = col_inner.width as usize;
                let lines: Vec<Line> = content.lines()
                    .take(col_inner.height as usize)
                    .map(|l| {
                        let char_count = l.chars().count();
                        let truncated = if char_count > max_w && max_w > 0 {
                            let mut s: String = l.chars().take(max_w.saturating_sub(1)).collect();
                            s.push('\u{2026}');
                            s
                        } else {
                            l.to_string()
                        };
                        Line::from(Span::styled(truncated, Style::default().fg(theme.muted)))
                    })
                    .collect();
                frame.render_widget(Paragraph::new(lines), col_inner);
            } else {
                frame.render_widget(
                    Paragraph::new(Span::styled("(no content)", Style::default().fg(theme.muted))),
                    col_inner,
                );
            }
        } else {
            frame.render_widget(
                Paragraph::new(Span::styled("\u{2014} not configured \u{2014}", Style::default().fg(theme.muted))),
                col_inner,
            );
        }
    }

    let hint_y = inner.y + inner.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "  Press 1, 2, or 3 to choose   Esc skip (keep file as-is)",
            Style::default().fg(theme.muted),
        )),
        Rect { x: inner.x, y: hint_y, width: inner.width, height: 1 },
    );
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
