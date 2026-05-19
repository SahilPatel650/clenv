pub mod app;
pub mod events;
pub mod onboarding;
pub mod ui;

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
