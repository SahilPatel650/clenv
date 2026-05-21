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
use app::AppState;
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
                        EventOutcome::SaveShellModules => {
                            let to_enable: Vec<_> = app.shell.entries.iter()
                                .filter(|e| {
                                    let pending = app.shell.pending_enabled.get(&e.definition.name).copied().unwrap_or(e.enabled);
                                    pending && !e.enabled
                                })
                                .map(|e| e.definition.clone())
                                .collect();

                            let to_disable: Vec<_> = app.shell.entries.iter()
                                .filter(|e| {
                                    let pending = app.shell.pending_enabled.get(&e.definition.name).copied().unwrap_or(e.enabled);
                                    !pending && e.enabled
                                })
                                .map(|e| e.definition.clone())
                                .collect();

                            if to_enable.is_empty() && to_disable.is_empty() {
                                app.status_message = Some("No changes to save".to_string());
                            } else {
                                let zshrc_path = config.modules.zshrc_path.clone()
                                    .unwrap_or_else(|| app.home_dir.join(".zshrc"));

                                for module in &to_enable {
                                    let needs_install = matches!(
                                        crate::modules::detect::module_status(module, &zshrc_path),
                                        crate::modules::ModuleStatus::NotInstalled
                                    );

                                    if needs_install {
                                        if let Some(cmd) = crate::modules::installer::install_preview(module) {
                                            let _ = disable_raw_mode();
                                            let mut stdout = io::stdout();
                                            let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                                            let _ = terminal.show_cursor();
                                            println!("\r\n  Installing {}: {}\r\n", module.name, cmd);

                                            let status = std::process::Command::new("sh")
                                                .arg("-c")
                                                .arg(&cmd)
                                                .status();

                                            match status {
                                                Ok(s) if s.success() => println!("\r\n  ✓ {} installed\r\n", module.name),
                                                Ok(s) => println!("\r\n  ✗ {} install failed (exit {})\r\n", module.name, s),
                                                Err(e) => println!("\r\n  ✗ {} install error: {}\r\n", module.name, e),
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
                                    }

                                    if !module.zshrc.snippet.is_empty() {
                                        let _ = crate::modules::zshrc::write_block(&zshrc_path, &module.name, &module.zshrc.snippet);
                                    }
                                    if !config.modules.enabled.contains(&module.name) {
                                        config.modules.enabled.push(module.name.clone());
                                    }
                                }

                                for module in &to_disable {
                                    let _ = crate::modules::zshrc::remove_block(&zshrc_path, &module.name);
                                    config.modules.enabled.retain(|n| n != &module.name);
                                }

                                let _ = crate::config::save(config);
                                app.load_shell_modules(&config.modules);

                                let n = to_enable.len() + to_disable.len();
                                app.status_message = Some(format!("{n} shell module(s) updated — reload your shell"));
                            }
                        }
                        EventOutcome::AdoptShellModule(name) => {
                            let zshrc_path = config.modules.zshrc_path.clone()
                                .unwrap_or_else(|| app.home_dir.join(".zshrc"));

                            if let Some(entry) = app.shell.entries.iter().find(|e| e.definition.name == name) {
                                let module = entry.definition.clone();
                                match crate::modules::zshrc::write_block(&zshrc_path, &module.name, &module.zshrc.snippet) {
                                    Ok(_) => {
                                        if !config.modules.enabled.contains(&module.name) {
                                            config.modules.enabled.push(module.name.clone());
                                        }
                                        let _ = crate::config::save(config);
                                        app.load_shell_modules(&config.modules);
                                        app.status_message = Some(format!("Adopted {} — reload your shell", name));
                                    }
                                    Err(e) => app.status_message = Some(format!("Adopt failed: {e}")),
                                }
                            }
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
