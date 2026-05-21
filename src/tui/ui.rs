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

pub fn render(frame: &mut Frame, app: &mut AppState) {
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
        render_shell_tab(frame, app, chunks[2], &theme);
        render_shell_detail(frame, app, chunks[3], &theme);
    } else {
        render_sort_bar(frame, app, chunks[1], &theme);
        render_table(frame, app, chunks[2], &theme);
        render_detail(frame, app, chunks[3], &theme);
    }

    render_status_bar(frame, app, chunks[4], &theme);

    if app.onboarding.is_some() {
        render_onboarding_overlay(frame, app, area, &theme);
    } else if app.show_tab_manager {
        render_tab_manager_overlay(frame, app, area, &theme);
    }
    if app.show_help {
        render_help_overlay(frame, area, &theme);
    }
    if app.confirm_delete {
        render_confirm_dialog(frame, app, area, &theme);
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
    let msg = if app.searching {
        Span::styled(
            "[Esc] exit search  [↑↓] navigate  [Backspace] delete char",
            Style::default().fg(theme.highlight),
        )
    } else if let Some(status) = &app.status_message {
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
        Span::raw(
            "[Space] toggle  [s] save  [a] adopt  [c] AI context  [?] help  [q] quit",
        )
    } else {
        Span::raw(
            "[d] delete  [c] cache  [a] activate  [y] copy  [r] refresh  [/] search  [⚙] tabs  [?] help  [q] quit",
        )
    };
    frame.render_widget(Paragraph::new(Line::from(msg)), area);
}

fn render_shell_tab(frame: &mut Frame, app: &mut AppState, area: Rect, theme: &Theme) {
    use crate::modules::ModuleStatus;

    // Category display order
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

    // Build flat list items: (is_header, category_label | module_index)
    #[derive(Clone)]
    enum ListItem {
        Header(String),
        Module(usize), // index into app.shell.entries
    }

    // Collect entries grouped by category (stable order: CATEGORY_ORDER first, then unknown)
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
    // deduplicate (BTreeSet already unique, but sort may not preserve insertion)
    categories.dedup();

    let mut flat_items: Vec<ListItem> = Vec::new();
    for cat in &categories {
        flat_items.push(ListItem::Header(cat.clone()));
        for (idx, entry) in app.shell.entries.iter().enumerate() {
            if &entry.definition.category == cat {
                flat_items.push(ListItem::Module(idx));
            }
        }
    }

    // Count only module items for cursor bounds
    let module_count = app.shell.entries.len();

    let block = Block::default().borders(Borders::TOP | Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let max_visible_rows = inner.height as usize;

    // Map cursor (module index) to flat_items position
    // scroll_offset tracks flat-list rows, not just modules
    // Simpler: compute visible flat rows starting at scroll_offset
    let total_flat = flat_items.len();
    let scroll = app.shell.scroll_offset.min(total_flat.saturating_sub(1));

    let mut item_rects: Vec<HitRect> = Vec::new();
    // Resize to module_count slots
    item_rects.resize(module_count, HitRect::default());

    // flat_cursor: position in flat_items of the currently selected module
    let flat_cursor = flat_items.iter().position(|item| {
        matches!(item, ListItem::Module(i) if *i == app.shell.cursor)
    }).unwrap_or(0);

    // Compute which flat rows are visible
    let visible_start = scroll;
    let visible_end = (visible_start + max_visible_rows).min(total_flat);

    let mut row_y = inner.y;
    for flat_idx in visible_start..visible_end {
        let item = &flat_items[flat_idx];
        match item {
            ListItem::Header(cat) => {
                let label = cat.to_uppercase().replace('-', " ");
                let line = Line::from(Span::styled(
                    format!(" {label}"),
                    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
                ));
                let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };
                frame.render_widget(Paragraph::new(line), row_area);
                row_y += 1;
            }
            ListItem::Module(idx) => {
                let entry = &app.shell.entries[*idx];
                let name = &entry.definition.name;
                let desc = &entry.definition.description;
                let pending = app.shell.pending_enabled.get(name.as_str())
                    .copied()
                    .unwrap_or(entry.enabled);
                let checkbox = if pending { "[✓]" } else { "[ ]" };
                let status_label = entry.status.label();
                let status_color = match &entry.status {
                    ModuleStatus::ManagedActive => theme.ok,
                    ModuleStatus::InstalledUnmanaged => theme.warn,
                    ModuleStatus::NotInstalled => theme.muted,
                    ModuleStatus::ManagedInactive => theme.muted,
                };

                let is_selected = *idx == app.shell.cursor;
                let row_style = if is_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };

                // Truncate / pad name and description
                let name_padded = format!("{:<15}", if name.len() > 15 { &name[..15] } else { name });
                let desc_str: String = if desc.len() > 35 {
                    desc[..35].to_string()
                } else {
                    format!("{:<35}", desc)
                };

                let row_area = Rect { x: inner.x, y: row_y, width: inner.width, height: 1 };

                // Build spans
                let mut spans = vec![
                    Span::styled(format!("   {} ", checkbox), row_style),
                    Span::styled(name_padded, row_style),
                    Span::raw("  "),
                    Span::styled(desc_str, row_style),
                    Span::raw("  "),
                ];

                // Status uses color override unless selected (reversed handles bg)
                if is_selected {
                    spans.push(Span::styled(status_label, row_style));
                } else {
                    spans.push(Span::styled(status_label, Style::default().fg(status_color)));
                }

                frame.render_widget(Paragraph::new(Line::from(spans)), row_area);

                // Store hit rect
                item_rects[*idx] = HitRect {
                    x: inner.x,
                    y: row_y,
                    width: inner.width,
                    height: 1,
                };

                row_y += 1;
            }
        }
    }

    app.shell.item_rects = item_rects;

    // Auto-scroll to keep cursor visible
    // If flat_cursor is above visible window, scroll up
    if flat_cursor < app.shell.scroll_offset {
        app.shell.scroll_offset = flat_cursor;
    }
    // If flat_cursor is below visible window, scroll down
    if flat_cursor >= app.shell.scroll_offset + max_visible_rows {
        app.shell.scroll_offset = flat_cursor + 1 - max_visible_rows;
    }
}

fn render_shell_detail(frame: &mut Frame, app: &AppState, area: Rect, theme: &Theme) {
    let block = Block::default().borders(Borders::ALL);

    let Some(entry) = app.selected_module() else {
        frame.render_widget(block, area);
        return;
    };

    let def = &entry.definition;
    let block = block.title(format!(" {} ", def.name));

    let status_label = entry.status.label();
    let startup_ms = def.zshrc.startup_ms_estimate;
    let depends = if def.depends_on.is_empty() {
        "none".to_string()
    } else {
        def.depends_on.join(", ")
    };
    let user_extend = def.zshrc.user_extend.as_deref().unwrap_or("not set");

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
            Span::raw(format!("~{startup_ms}ms")),
        ]),
        Line::from(vec![
            Span::styled("Depends on:   ", Style::default().fg(theme.highlight)),
            Span::raw(depends),
        ]),
        Line::from(vec![
            Span::styled("zshrc order:  ", Style::default().fg(theme.highlight)),
            Span::raw(def.zshrc.order.to_string()),
        ]),
        Line::from(vec![
            Span::styled("User extend:  ", Style::default().fg(theme.highlight)),
            Span::raw(user_extend.to_string()),
        ]),
    ];

    if let Some(sync_time) = app.shell.private_repo_last_sync {
        let label = format_age(Some(sync_time));
        text.push(Line::from(vec![
            Span::styled("Private repo: ", Style::default().fg(theme.highlight)),
            Span::raw(format!("synced {label}")),
        ]));
    }

    frame.render_widget(Paragraph::new(text).block(block), area);
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
        Line::from("  q                 Quit"),
    ];
    let block = popup_block(" Help ", theme);
    frame.render_widget(Paragraph::new(help_text).block(block), popup_area);
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
