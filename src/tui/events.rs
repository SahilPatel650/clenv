use crate::actions;
use crate::tui::app::AppState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use humansize::{format_size, BINARY};

pub enum EventOutcome {
    Continue,
    Quit,
    PrintActivation(String),
    Refresh,
}

pub fn handle_key(key: KeyEvent, app: &mut AppState) -> EventOutcome {
    // Confirm-delete mode
    if app.confirm_delete {
        return match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(env) = app.selected_env() {
                    let env = env.clone();
                    match actions::delete_env(&env) {
                        Ok(freed) => {
                            app.envs.retain(|e| e.path != env.path);
                            app.status_message = Some(format!(
                                "Deleted {} — freed {}",
                                env.name,
                                format_size(freed, BINARY)
                            ));
                            if app.selected > 0 {
                                app.selected -= 1;
                            }
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
                app.confirm_delete = false;
                EventOutcome::Continue
            }
            _ => {
                app.confirm_delete = false;
                app.status_message = Some("Delete cancelled".to_string());
                EventOutcome::Continue
            }
        };
    }

    match key.code {
        KeyCode::Char('q') => EventOutcome::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => EventOutcome::Quit,

        KeyCode::Tab => {
            app.next_tab();
            EventOutcome::Continue
        }
        KeyCode::BackTab => {
            app.prev_tab();
            EventOutcome::Continue
        }

        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            EventOutcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            EventOutcome::Continue
        }

        KeyCode::Char('s') => {
            app.cycle_sort();
            EventOutcome::Continue
        }

        KeyCode::Esc => {
            app.search.clear();
            app.status_message = None;
            EventOutcome::Continue
        }

        KeyCode::Backspace => {
            app.search.pop();
            app.selected = 0;
            EventOutcome::Continue
        }

        KeyCode::Char('?') => {
            app.show_help = !app.show_help;
            EventOutcome::Continue
        }

        KeyCode::Char('r') => EventOutcome::Refresh,

        KeyCode::Char('d') => {
            if app.selected_env().is_some() {
                app.confirm_delete = true;
            }
            EventOutcome::Continue
        }

        KeyCode::Char('c') => {
            if let Some(env) = app.selected_env() {
                let env = env.clone();
                match actions::clear_cache(&env) {
                    Ok(freed) => {
                        app.status_message = Some(format!(
                            "Cache cleared — freed {}",
                            format_size(freed, BINARY)
                        ));
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Error: {e}"));
                    }
                }
            }
            EventOutcome::Continue
        }

        KeyCode::Char('a') => {
            if let Some(env) = app.selected_env() {
                if let Some(cmd) = env.activation_cmd.clone() {
                    return EventOutcome::PrintActivation(cmd);
                } else {
                    app.status_message =
                        Some("No activation command for this env".to_string());
                }
                EventOutcome::Continue
            } else {
                // No env selected — treat as search character
                app.search.push('a');
                app.selected = 0;
                EventOutcome::Continue
            }
        }

        KeyCode::Char('y') => {
            if let Some(env) = app.selected_env() {
                if let Some(cmd) = env.activation_cmd.clone() {
                    match actions::copy_to_clipboard(&cmd) {
                        Ok(_) => {
                            app.status_message =
                                Some("Activation command copied to clipboard".to_string())
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Clipboard error: {e}"))
                        }
                    }
                } else {
                    app.status_message =
                        Some("No activation command for this env".to_string());
                }
            }
            EventOutcome::Continue
        }

        // Any other printable character goes into search
        KeyCode::Char(c) => {
            app.search.push(c);
            app.selected = 0;
            EventOutcome::Continue
        }

        _ => EventOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{EnvKind, Environment, HealthStatus};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn make_app(envs: Vec<Environment>) -> AppState {
        AppState::new(envs, "All", "size")
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn make_env(name: &str) -> Environment {
        let mut e = Environment::new(EnvKind::Python, PathBuf::from(format!("/fake/{name}")));
        e.name = name.to_string();
        e.health = HealthStatus::Ok;
        e
    }

    #[test]
    fn q_key_returns_quit() {
        let mut app = make_app(vec![]);
        let outcome = handle_key(key(KeyCode::Char('q')), &mut app);
        assert!(matches!(outcome, EventOutcome::Quit));
    }

    #[test]
    fn r_key_returns_refresh() {
        let mut app = make_app(vec![]);
        let outcome = handle_key(key(KeyCode::Char('r')), &mut app);
        assert!(matches!(outcome, EventOutcome::Refresh));
    }

    #[test]
    fn search_chars_accumulate() {
        let mut app = make_app(vec![]);
        handle_key(key(KeyCode::Char('a')), &mut app);
        handle_key(key(KeyCode::Char('p')), &mut app);
        handle_key(key(KeyCode::Char('i')), &mut app);
        assert_eq!(app.search, "api");
    }

    #[test]
    fn esc_clears_search() {
        let mut app = make_app(vec![]);
        app.search = "hello".to_string();
        handle_key(key(KeyCode::Esc), &mut app);
        assert!(app.search.is_empty());
    }

    #[test]
    fn d_sets_confirm_delete_when_env_selected() {
        let mut app = make_app(vec![make_env("myenv")]);
        handle_key(key(KeyCode::Char('d')), &mut app);
        assert!(app.confirm_delete);
    }

    #[test]
    fn confirm_delete_n_cancels() {
        let mut app = make_app(vec![make_env("myenv")]);
        app.confirm_delete = true;
        handle_key(key(KeyCode::Char('n')), &mut app);
        assert!(!app.confirm_delete);
        assert!(app.status_message.as_deref() == Some("Delete cancelled"));
    }

    #[test]
    fn a_key_returns_activation_cmd() {
        let mut env = make_env("myenv");
        env.activation_cmd = Some("source /fake/myenv/bin/activate".to_string());
        let mut app = make_app(vec![env]);
        let outcome = handle_key(key(KeyCode::Char('a')), &mut app);
        assert!(matches!(outcome, EventOutcome::PrintActivation(_)));
    }
}
