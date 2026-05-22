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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::EventOutcome;
use humansize::{format_size, BINARY};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::mpsc, time::Duration};

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
    mut scan_rx: Option<mpsc::Receiver<Vec<Environment>>>,
) -> Result<Option<String>> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        // Poll background scan result without blocking
        if let Some(rx) = &scan_rx {
            match rx.try_recv() {
                Ok(new_envs) => app.update_envs(new_envs),
                Err(mpsc::TryRecvError::Disconnected) => app.rescanning = false,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    match events::handle_key(key, app) {
                        EventOutcome::Quit => return Ok(None),
                        EventOutcome::PrintActivation(cmd) => return Ok(Some(cmd)),
                        EventOutcome::Refresh => {
                            app.status_message = Some("Scanning…".to_string());
                            terminal.draw(|f| ui::render(f, app))?;
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
                                    // Read one keypress before resuming the TUI
                                    let _ = enable_raw_mode();
                                    loop {
                                        if event::poll(Duration::from_secs(60))? {
                                            if let Event::Key(_) = event::read()? {
                                                break;
                                            }
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
                                                if event::poll(Duration::from_secs(60))? {
                                                    if let Event::Key(_) = event::read()? { break; }
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
                                } else {
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
                                    if event::poll(Duration::from_secs(60))? {
                                        if let Event::Key(_) = event::read()? { break; }
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
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || {
                            let envs = scanner::scan(&scan_cfg);
                            let _ = crate::config::cache::save(&envs);
                            let _ = tx.send(envs);
                        });
                        scan_rx = Some(rx);
                        app.rescanning = true;
                        app.status_message = Some("Settings saved — rescanning…".to_string());
                    }
                }
                Event::Mouse(mouse) => {
                    events::handle_mouse(mouse, app);
                }
                _ => {}
            }
        }
    }
}
