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

/// Run the TUI. Returns Some(activation_cmd) if user pressed `a`.
pub fn run(envs: Vec<Environment>, config: &Config) -> Result<Option<String>> {
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

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    config: &Config,
) -> Result<Option<String>> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
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
        }
    }
}
