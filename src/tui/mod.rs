pub mod app;
pub mod events;
pub mod onboarding;
pub mod theme;
pub mod ui;

use crate::actions;
use crate::config::Config;
use crate::env::Environment;
use crate::scanner;
use anyhow::Result;
use app::{AppState, BaseDepsOverlay};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::EventOutcome;
use humansize::{format_size, BINARY};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::Duration};

enum TuiEvent {
    Crossterm(crossterm::event::Event),
    ZshrcChanged(crate::tui::app::ChangedBlock),
    ScanResult(Vec<crate::env::Environment>),
}

fn spawn_zshrc_watcher(
    zshrc_path: std::path::PathBuf,
    tx: mpsc::Sender<TuiEvent>,
) {
    std::thread::spawn(move || {
        use crate::modules::zshrc::{parse_segments, SegmentKind};
        use std::time::{Duration, SystemTime};

        let mut last_mtime = std::fs::metadata(&zshrc_path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut last_segments = parse_segments(&zshrc_path);

        loop {
            std::thread::sleep(Duration::from_secs(2));

            let mtime = match std::fs::metadata(&zshrc_path).and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            if mtime <= last_mtime { continue; }
            last_mtime = mtime;

            let new_segments = parse_segments(&zshrc_path);

            for seg in &new_segments {
                let already_known = last_segments.iter().any(|s| match (&s.kind, &seg.kind) {
                    (SegmentKind::Clenv(a), SegmentKind::Clenv(b)) => a == b && s.content == seg.content,
                    (SegmentKind::Unmanaged, SegmentKind::Unmanaged) => s.content == seg.content,
                    _ => false,
                });
                if !already_known {
                    let block = crate::tui::app::ChangedBlock {
                        name: match &seg.kind {
                            SegmentKind::Clenv(n) => Some(n.clone()),
                            SegmentKind::Unmanaged => None,
                        },
                        new_content: seg.content.clone(),
                        canonical_content: None,
                        custom_content: None,
                    };
                    if tx.send(TuiEvent::ZshrcChanged(block)).is_err() { break; }
                }
            }
            last_segments = new_segments;
        }
    });
}

fn spawn_scan_forwarder(scan_rx: mpsc::Receiver<Vec<Environment>>, tx: mpsc::Sender<TuiEvent>) {
    std::thread::spawn(move || {
        for envs in scan_rx {
            if tx.send(TuiEvent::ScanResult(envs)).is_err() { break; }
        }
    });
}

/// Run the TUI. Returns (activation_cmd, final app state).
///
/// - `scan_rx`: receives a fresh env list from the background scanner; `None` if no background scan.
/// - `rescanning`: `true` when initial envs came from cache.
/// - `first_run`: when `true`, the onboarding overlay is shown on startup.
pub fn run(
    envs: Vec<Environment>,
    config: &mut Config,
    scan_rx: Option<mpsc::Receiver<Vec<Environment>>>,
    rescanning: bool,
    first_run: bool,
) -> Result<(Option<String>, AppState)> {
    let mut app = AppState::new(
        envs,
        &config.session.last_tab,
        &config.session.last_sort,
    );
    app.scroll_offset = config.session.last_scroll;
    app.rescanning = rescanning;
    app.load_shell_modules(config);

    if first_run {
        app.onboarding = Some(onboarding::OnboardingState::new(
            &config.scan.roots,
            config.scan.depth_limit,
        ));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app, config, scan_rx);

    // Always run all teardown steps — use ? would skip subsequent steps on failure
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal.show_cursor();

    let activation_cmd = result?;

    if app.zshrc_modified_this_session {
        use std::io::Write as _;
        print!("\n  ~/.zshrc was modified this session.\n  Source it now to apply changes? [y/N]: ");
        let _ = io::stdout().flush();

        let _ = crossterm::terminal::enable_raw_mode();
        let sourced = loop {
            if crossterm::event::poll(std::time::Duration::from_secs(30)).unwrap_or(false) {
                if let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read() {
                    break matches!(k.code, crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y'));
                }
            } else {
                break false;
            }
        };
        let _ = crossterm::terminal::disable_raw_mode();
        println!();

        if sourced {
            println!("source ~/.zshrc");
        }
    }

    Ok((activation_cmd, app))
}

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    config: &mut Config,
    scan_rx: Option<mpsc::Receiver<Vec<Environment>>>,
) -> Result<Option<String>> {
    let (tx, rx) = mpsc::channel::<TuiEvent>();

    // Crossterm reader thread
    let ct_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            if let Ok(ev) = crossterm::event::read() {
                if ct_tx.send(TuiEvent::Crossterm(ev)).is_err() { break; }
            }
        }
    });

    // Scan result forwarder thread
    if let Some(srx) = scan_rx {
        spawn_scan_forwarder(srx, tx.clone());
    }

    // zshrc watcher thread
    if config.modules.watch_zshrc {
        let zshrc_path = config.modules.zshrc_path.clone()
            .unwrap_or_else(|| app.home_dir.join(".zshrc"));
        spawn_zshrc_watcher(zshrc_path, tx.clone());
    }

    loop {
        terminal.draw(|f| ui::render(f, app, config))?;

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(TuiEvent::ScanResult(envs)) => {
                app.update_envs(envs);
            }
            Ok(TuiEvent::ZshrcChanged(mut block)) => {
                if let Some(ref name) = block.name.clone() {
                    block.canonical_content = app.shell.entries.iter()
                        .find(|e| &e.definition.name == name)
                        .map(|e| e.definition.zshrc.snippet.clone());
                    let private_path = app.home_dir
                        .join(".config/clenv/private")
                        .join(format!("{name}.zsh"));
                    block.custom_content = std::fs::read_to_string(&private_path).ok();
                }
                app.zshrc_change_modal = Some(crate::tui::app::ZshrcChangeModal { block, selected: 2 });
            }
            Ok(TuiEvent::Crossterm(ev)) => {
                match ev {
                    Event::Key(key) => {
                        match events::handle_key(key, app) {
                            EventOutcome::Quit => return Ok(None),
                            EventOutcome::PrintActivation(cmd) => return Ok(Some(cmd)),
                            EventOutcome::Refresh => {
                                app.status_message = Some("Scanning…".to_string());
                                terminal.draw(|f| ui::render(f, app, config))?;
                                let new_envs = scanner::scan(&config.scan);
                                app.envs = new_envs;
                                app.selected = 0;
                                app.status_message =
                                    Some(format!("Found {} environments", app.envs.len()));
                            }
                            EventOutcome::DeleteConfirmed => {
                                if let Some(env) = app.selected_env().cloned() {
                                    let streams = actions::delete_streams_output(&env);
                                    if streams {
                                        // Suspend TUI so the manager's output flows to the terminal
                                        let _ = disable_raw_mode();
                                        let mut stdout = io::stdout();
                                        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                                        let _ = terminal.show_cursor();
                                        println!("\r\n  Running: {}\r\n", actions::delete_preview(&env));
                                    }

                                    match actions::delete_env(&env) {
                                        Ok(freed) => {
                                            let msg = format!(
                                                "Deleted {} — freed {}",
                                                env.name,
                                                format_size(freed, BINARY)
                                            );
                                            if streams {
                                                println!("\r\n  ✓ {msg}\r\n");
                                            }
                                            app.envs.retain(|e| e.path != env.path);
                                            if app.selected > 0 {
                                                app.selected -= 1;
                                            }
                                            app.status_message = Some(msg);
                                        }
                                        Err(e) => {
                                            let msg = format!("Error: {e}");
                                            if streams {
                                                eprintln!("\r\n  ✗ {msg}\r\n");
                                            }
                                            app.status_message = Some(msg);
                                        }
                                    }

                                    if streams {
                                        println!("  Press any key to return…\r");
                                        let _ = enable_raw_mode();
                                        // Use rx so the reader thread captures the key and we
                                        // consume it here rather than re-processing it in the
                                        // main event loop.
                                        loop {
                                            match rx.recv_timeout(Duration::from_secs(60)) {
                                                Ok(TuiEvent::Crossterm(Event::Key(_))) => break,
                                                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                                            }
                                        }
                                        let mut stdout = io::stdout();
                                        let _ = execute!(stdout, EnterAlternateScreen, EnableMouseCapture);
                                        terminal.clear()?;
                                    }
                                }
                                app.confirm_delete = false;
                            }
                            EventOutcome::InstallModule(name) => {
                                // First-install-per-session: warn if base system deps are missing
                                if !app.base_deps_checked {
                                    let missing: Vec<String> = ["git", "curl", "wget", "zsh"]
                                        .iter()
                                        .filter(|dep| {
                                            std::process::Command::new("which")
                                                .arg(dep)
                                                .output()
                                                .map(|o| !o.status.success())
                                                .unwrap_or(true)
                                        })
                                        .map(|s| s.to_string())
                                        .collect();
                                    if !missing.is_empty() {
                                        app.base_deps_overlay = Some(BaseDepsOverlay {
                                            missing,
                                            pending_name: name.clone(),
                                        });
                                        continue; // don't proceed with install yet
                                    }
                                    app.base_deps_checked = true;
                                }

                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));

                                if let Some(entry) = app.shell.entries.iter().find(|e| e.definition.name == name).cloned() {
                                    let module = entry.definition.clone();
                                    let needs_install = matches!(entry.status, crate::modules::ModuleStatus::NotInstalled);
                                    let mut ready_to_enable = !needs_install;

                                    if needs_install {
                                        match crate::modules::installer::install_preview(&module) {
                                            Some(cmd) => {
                                                // Suspend TUI — install scripts need a real terminal
                                                // (sudo prompts, interactive wizards, etc.)
                                                let _ = disable_raw_mode();
                                                let mut stdout = io::stdout();
                                                let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                                                let _ = terminal.show_cursor();

                                                println!("\r\n  Installing {}…\r\n", module.name);
                                                println!("  $ {}\r\n", cmd);

                                                let exit_status = std::process::Command::new("sh")
                                                    .arg("-c")
                                                    .arg(&cmd)
                                                    .status();

                                                let detected = crate::modules::detect::is_installed(&module);

                                                match (&exit_status, detected) {
                                                    (Ok(s), true) if s.success() => {
                                                        println!("\r\n  ✓ {} installed and detected\r\n", module.name);
                                                        ready_to_enable = true;
                                                    }
                                                    (Ok(s), false) if s.success() => {
                                                        println!("\r\n  ⚠ {} script succeeded — reload shell to activate\r\n", module.name);
                                                        ready_to_enable = true;
                                                    }
                                                    (Ok(s), _) => {
                                                        println!("\r\n  ✗ {} install failed (exit {})\r\n", module.name, s);
                                                    }
                                                    (Err(e), _) => {
                                                        println!("\r\n  ✗ {} install error: {e}\r\n", module.name);
                                                    }
                                                }

                                                println!("  Press any key to return…\r");
                                                let _ = enable_raw_mode();
                                                loop {
                                                    match rx.recv_timeout(Duration::from_secs(60)) {
                                                        Ok(TuiEvent::Crossterm(Event::Key(_))) => break,
                                                        Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                                                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                                                    }
                                                }
                                                let mut stdout = io::stdout();
                                                let _ = execute!(stdout, EnterAlternateScreen, EnableMouseCapture);
                                                terminal.clear()?;
                                            }
                                            None => {
                                                app.status_message = Some(format!("No installer for {name} on this platform"));
                                            }
                                        }
                                    }

                                    // Check for custom snippet in private repo
                                    let private_snippet_path = app.home_dir
                                        .join(".config/clenv/private")
                                        .join(format!("{}.zsh", module.name));
                                    let custom_content = std::fs::read_to_string(&private_snippet_path).ok();

                                    // Detect if install script added new content to zshrc
                                    let segments_after = crate::modules::zshrc::parse_segments(&zshrc_path);
                                    let install_script_content: Option<String> = segments_after.iter()
                                        .filter_map(|s| match &s.kind {
                                            crate::modules::zshrc::SegmentKind::Unmanaged => Some(s.content.clone()),
                                            _ => None,
                                        })
                                        .find(|content| content.to_lowercase().contains(&module.name.to_lowercase()));

                                    if ready_to_enable && (custom_content.is_some() || install_script_content.is_some()) {
                                        let block = crate::tui::app::ChangedBlock {
                                            name: Some(module.name.clone()),
                                            new_content: install_script_content.unwrap_or_default(),
                                            canonical_content: if !module.zshrc.snippet.is_empty() {
                                                Some(module.zshrc.snippet.clone())
                                            } else {
                                                None
                                            },
                                            custom_content,
                                        };
                                        let preferred_is_private = config.modules.preferred_snippet_source
                                            == crate::config::SnippetSource::PrivateRepo;
                                        let has_custom = block.custom_content.is_some();
                                        app.zshrc_change_modal = Some(crate::tui::app::ZshrcChangeModal {
                                            block,
                                            selected: if preferred_is_private && has_custom { 3 } else { 2 },
                                        });
                                        // Don't write the zshrc block now — ZshrcChangeResolved handler will do it
                                        ready_to_enable = false;
                                    }

                                    if ready_to_enable {
                                        if !module.zshrc.snippet.is_empty() {
                                            let _ = crate::modules::zshrc::write_block(
                                                &zshrc_path,
                                                &module.name,
                                                &module.zshrc.snippet,
                                            );
                                            app.zshrc_modified_this_session = true;
                                        }
                                        if !config.modules.enabled.contains(&module.name) {
                                            config.modules.enabled.push(module.name.clone());
                                        }
                                        let _ = crate::config::save(config);
                                        app.status_message = Some(format!("✓ {name} enabled — reload your shell"));
                                    } else if app.zshrc_change_modal.is_none() {
                                        app.status_message = Some(format!("✗ {name} install failed"));
                                    }
                                    if config.ui.auto_detect_after_install {
                                        app.load_shell_modules(config);
                                    }
                                }
                            }
                            EventOutcome::DisableModule(name) => {
                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                                let _ = crate::modules::zshrc::remove_block(&zshrc_path, &name);
                                app.zshrc_modified_this_session = true;
                                config.modules.enabled.retain(|n| n != &name);
                                let _ = crate::config::save(config);
                                app.load_shell_modules(config);
                                app.status_message = Some(format!("✓ {name} disabled — reload your shell"));
                            }
                            EventOutcome::CopyShellContext => {
                                let mut ctx = String::new();
                                ctx.push_str("# clenv Module Context\n\n");
                                ctx.push_str("## Available modules and their current status:\n\n");
                                for entry in &app.shell.entries {
                                    ctx.push_str(&format!(
                                        "- {} [{}]: {}\n",
                                        entry.definition.name,
                                        entry.status.label(),
                                        entry.definition.description
                                    ));
                                }

                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                                if let Ok(zshrc) = std::fs::read_to_string(&zshrc_path) {
                                    ctx.push_str("\n## Current ~/.zshrc:\n\n```zsh\n");
                                    ctx.push_str(&zshrc);
                                    ctx.push_str("\n```\n");
                                }

                                match actions::copy_to_clipboard(&ctx) {
                                    Ok(_) => app.status_message = Some("AI context copied to clipboard".to_string()),
                                    Err(e) => app.status_message = Some(format!("Clipboard error: {e}")),
                                }
                            }
                            EventOutcome::SyncPrivateRepo => {
                                if let Some(repo_url) = &config.modules.private_dotfiles_repo.clone() {
                                    let private_dir = dirs::home_dir()
                                        .unwrap_or_default()
                                        .join(".config/clenv/private");

                                    // Suspend TUI for streaming output
                                    let _ = disable_raw_mode();
                                    let mut stdout = io::stdout();
                                    let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                                    let _ = terminal.show_cursor();

                                    println!("\r\n  Syncing private repo: {repo_url}\r\n");

                                    match crate::modules::private_repo::sync(repo_url, &private_dir) {
                                        Ok(_) => {
                                            println!("\r\n  \u{2713} Private repo synced\r\n");
                                            app.shell.private_repo_last_sync = Some(std::time::SystemTime::now());
                                        }
                                        Err(e) => println!("\r\n  \u{2717} Sync failed: {e}\r\n"),
                                    }

                                    println!("  Press any key to return\u{2026}\r");
                                    let _ = enable_raw_mode();
                                    loop {
                                        match rx.recv_timeout(Duration::from_secs(60)) {
                                            Ok(TuiEvent::Crossterm(Event::Key(_))) => break,
                                            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                                        }
                                    }
                                    let mut stdout = io::stdout();
                                    let _ = execute!(stdout, EnterAlternateScreen, EnableMouseCapture);
                                    terminal.clear()?;
                                } else {
                                    app.status_message = Some(
                                        "No private repo configured \u{2014} add private_dotfiles_repo to config.toml".to_string()
                                    );
                                }
                            }
                            EventOutcome::CreateBlock { name, description, after_block } => {
                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                                let _ = crate::modules::zshrc::write_block_at(
                                    &zshrc_path,
                                    &name,
                                    &format!("# (add your configuration for {name} here)"),
                                    after_block.as_deref(),
                                );
                                app.zshrc_modified_this_session = true;
                                // Store metadata in config so the block shows up with its description
                                config.modules.blocks.insert(name.clone(), crate::config::BlockMeta {
                                    description: if description.is_empty() { None } else { Some(description.clone()) },
                                    startup_ms: None,
                                });
                                let _ = crate::config::save(config);
                                app.load_shell_modules(config);
                                app.status_message = Some(format!(
                                    "Created block '{name}' — add your config in ~/.zshrc"
                                ));
                            }
                            EventOutcome::OpenSettings => {
                                let st = &mut app.settings_state;
                                if let Some(row) = st.editing {
                                    st.input_buf = match st.tab {
                                        crate::tui::app::SettingsTab::Shell => match row {
                                            0 => config.modules.zshrc_path.as_deref()
                                                    .map(|p| p.to_string_lossy().into_owned())
                                                    .unwrap_or_default(),
                                            1 => config.modules.private_dotfiles_repo.clone().unwrap_or_default(),
                                            2 => config.modules.agent_context_repo.clone().unwrap_or_default(),
                                            _ => String::new(),
                                        },
                                        crate::tui::app::SettingsTab::Scan => match row {
                                            0 => config.scan.depth_limit.to_string(),
                                            _ => config.scan.roots.get(row.saturating_sub(1))
                                                    .map(|p| p.to_string_lossy().into_owned())
                                                    .unwrap_or_default(),
                                        },
                                        crate::tui::app::SettingsTab::Ui => match row {
                                            0 => config.ui.default_tab.clone(),
                                            1 => config.ui.default_sort.clone(),
                                            2 => config.ui.default_sort_dir.clone(),
                                            _ => String::new(),
                                        },
                                    };
                                }
                            }

                            EventOutcome::SaveSettings => {
                                let tab = app.settings_state.tab;
                                let cursor = app.settings_state.cursor;
                                let editing = app.settings_state.editing;
                                let input = app.settings_state.input_buf.trim().to_string();

                                if tab == crate::tui::app::SettingsTab::Shell && editing.is_none() {
                                    match cursor {
                                        3 => config.ui.auto_detect_after_install = !config.ui.auto_detect_after_install,
                                        4 => config.modules.watch_zshrc = !config.modules.watch_zshrc,
                                        _ => {}
                                    }
                                }

                                if editing.is_some() && !input.is_empty() {
                                    match tab {
                                        crate::tui::app::SettingsTab::Shell => match cursor {
                                            0 => config.modules.zshrc_path = Some(std::path::PathBuf::from(&input)),
                                            1 => config.modules.private_dotfiles_repo = Some(input.clone()),
                                            2 => config.modules.agent_context_repo = Some(input.clone()),
                                            _ => {}
                                        },
                                        crate::tui::app::SettingsTab::Scan => match cursor {
                                            0 => { if let Ok(n) = input.parse::<usize>() { config.scan.depth_limit = n; } }
                                            n => {
                                                let idx = n.saturating_sub(1);
                                                if idx < config.scan.roots.len() {
                                                    config.scan.roots[idx] = std::path::PathBuf::from(&input);
                                                }
                                            }
                                        },
                                        crate::tui::app::SettingsTab::Ui => match cursor {
                                            0 => config.ui.default_tab = input.clone(),
                                            1 => config.ui.default_sort = input.clone(),
                                            2 => config.ui.default_sort_dir = input.clone(),
                                            _ => {}
                                        },
                                    }
                                    app.settings_state.input_buf.clear();
                                }

                                if !app.show_settings {
                                    let _ = crate::config::save(config);
                                }
                            }

                            EventOutcome::MoveBlock { from_idx, to_idx } => {
                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                                match crate::modules::zshrc::move_block(&zshrc_path, from_idx, to_idx) {
                                    Ok(()) => {
                                        app.zshrc_modified_this_session = true;
                                        app.load_shell_modules(config);
                                        app.status_message = Some("Block moved".to_string());
                                    }
                                    Err(e) => {
                                        app.status_message = Some(format!("Move failed: {e}"));
                                    }
                                }
                            }

                            EventOutcome::ZshrcChangeResolved { choice, block } => {
                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                                let result = match choice {
                                    1 => Ok(()), // keep install script version — no action
                                    2 => {
                                        if let (Some(name), Some(canonical)) = (&block.name, &block.canonical_content) {
                                            crate::modules::zshrc::write_block(&zshrc_path, name, canonical)
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    3 => {
                                        if let (Some(name), Some(custom)) = (&block.name, &block.custom_content) {
                                            crate::modules::zshrc::write_block(&zshrc_path, name, custom)
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    _ => Ok(()),
                                };
                                match result {
                                    Ok(()) => {
                                        app.zshrc_modified_this_session = true;
                                        if let Some(name) = &block.name {
                                            if !config.modules.enabled.contains(name) {
                                                config.modules.enabled.push(name.clone());
                                            }
                                            let _ = crate::config::save(config);
                                        }
                                        app.load_shell_modules(config);
                                    }
                                    Err(e) => {
                                        app.status_message = Some(format!("Error applying config: {e}"));
                                    }
                                }
                            }

                            EventOutcome::Continue => {}
                        }

                        // Apply onboarding result immediately after it's confirmed
                        if let Some(result) = app.onboarding_result.take() {
                            config.scan.roots = result.roots;
                            config.scan.depth_limit = result.depth_limit;
                            config.scan.ignore = result.ignore;
                            crate::config::save(config).ok();
                            // Rescan with the new settings
                            let scan_cfg = config.scan.clone();
                            let (stx, srx) = mpsc::channel();
                            std::thread::spawn(move || {
                                let envs = scanner::scan(&scan_cfg);
                                let _ = crate::config::cache::save(&envs);
                                let _ = stx.send(envs);
                            });
                            spawn_scan_forwarder(srx, tx.clone());
                            app.rescanning = true;
                            app.status_message = Some("Settings saved — rescanning…".to_string());
                        }
                    }
                    Event::Mouse(mouse) => {
                        if let EventOutcome::ZshrcChangeResolved { choice, block } =
                            events::handle_mouse(mouse, app)
                        {
                            let zshrc_path = config.modules.zshrc_path.clone()
                                .unwrap_or_else(|| app.home_dir.join(".zshrc"));
                            let result = match choice {
                                1 => Ok(()),
                                2 => {
                                    if let (Some(name), Some(canonical)) =
                                        (&block.name, &block.canonical_content)
                                    {
                                        crate::modules::zshrc::write_block(&zshrc_path, name, canonical)
                                    } else { Ok(()) }
                                }
                                3 => {
                                    if let (Some(name), Some(custom)) =
                                        (&block.name, &block.custom_content)
                                    {
                                        crate::modules::zshrc::write_block(&zshrc_path, name, custom)
                                    } else { Ok(()) }
                                }
                                _ => Ok(()),
                            };
                            match result {
                                Ok(()) => {
                                    app.zshrc_modified_this_session = true;
                                    if let Some(name) = &block.name {
                                        if !config.modules.enabled.contains(name) {
                                            config.modules.enabled.push(name.clone());
                                        }
                                        let _ = crate::config::save(config);
                                    }
                                    app.load_shell_modules(config);
                                }
                                Err(e) => {
                                    app.status_message = Some(format!("Error applying config: {e}"));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(None)
}
