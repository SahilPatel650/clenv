pub mod app;
pub mod events;
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
use std::{io, time::Duration};

/// Run the TUI. Returns (activation_cmd, final app state).
pub fn run(envs: Vec<Environment>, config: &Config) -> Result<(Option<String>, AppState)> {
    let mut app = AppState::new(
        envs,
        &config.session.last_tab,
        &config.session.last_sort,
    );
    app.scroll_offset = config.session.last_scroll;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app, config);

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
    config: &Config,
) -> Result<Option<String>> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

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
                }
                Event::Mouse(mouse) => {
                    events::handle_mouse(mouse, app);
                }
                _ => {}
            }
        }
    }
}
