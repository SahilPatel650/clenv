use crate::actions;
use crate::tui::app::{AppState, SortField, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use humansize::{format_size, BINARY};

pub enum EventOutcome {
    Continue,
    Quit,
    PrintActivation(String),
    Refresh,
    DeleteConfirmed,
    InstallModule(String),
    DisableModule(String),
    CopyShellContext,
    SyncPrivateRepo,
}

pub fn handle_key(key: KeyEvent, app: &mut AppState) -> EventOutcome {
    // Base deps warning intercepts all keys until dismissed
    if let Some(overlay) = &app.base_deps_overlay {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let name = overlay.pending_name.clone();
                app.base_deps_overlay = None;
                app.base_deps_checked = true;
                return EventOutcome::InstallModule(name);
            }
            _ => {
                app.base_deps_overlay = None;
            }
        }
        return EventOutcome::Continue;
    }

    if app.onboarding.is_some() {
        return handle_onboarding_key(key, app);
    }

    if app.confirm_delete {
        return match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => EventOutcome::DeleteConfirmed,
            _ => {
                app.confirm_delete = false;
                app.status_message = Some("Delete cancelled".to_string());
                EventOutcome::Continue
            }
        };
    }

    if app.show_tab_manager {
        return handle_tab_manager_key(key, app);
    }

    if app.searching {
        return handle_search_key(key, app);
    }

    if app.active_tab == Tab::Shell {
        match key.code {
            KeyCode::Char('1') => {
                app.active_tab = Tab::All;
                app.selected = 0;
                app.scroll_offset = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.shell.cursor > 0 {
                    app.shell.cursor -= 1;
                }
                // scroll_offset is a flat-list index (includes headers); let render handle it
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.shell.cursor + 1 < app.shell.entries.len() {
                    app.shell.cursor += 1;
                }
                // scroll_offset is a flat-list index (includes headers); let render handle it
            }
            KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Char('i') => {
                if let Some(entry) = app.selected_module() {
                    if !entry.missing_deps.is_empty() {
                        let missing = entry.missing_deps.join(", ");
                        app.status_message = Some(format!("Install deps first: {missing}"));
                    } else {
                        match &entry.status {
                            crate::modules::ModuleStatus::ManagedActive => {
                                let name = entry.definition.name.clone();
                                return EventOutcome::DisableModule(name);
                            }
                            _ => {
                                if entry.can_install
                                    || entry.status != crate::modules::ModuleStatus::NotInstalled
                                {
                                    let name = entry.definition.name.clone();
                                    return EventOutcome::InstallModule(name);
                                } else {
                                    app.status_message = Some(
                                        "No installer available for this platform".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(entry) = app.selected_module() {
                    let name = entry.definition.name.clone();
                    return EventOutcome::DisableModule(name);
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return EventOutcome::Quit;
            }
            KeyCode::Char('c') => return EventOutcome::CopyShellContext,
            KeyCode::Char('r') => return EventOutcome::SyncPrivateRepo,
            KeyCode::Esc => {
                app.status_message = None;
            }
            KeyCode::Char('?') => {
                app.show_help = !app.show_help;
            }
            KeyCode::Char('q') => return EventOutcome::Quit,
            KeyCode::Tab => app.next_tab(),
            KeyCode::BackTab => app.prev_tab(),
            _ => {}
        }
        return EventOutcome::Continue;
    }

    match key.code {
        KeyCode::Char('q') => EventOutcome::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => EventOutcome::Quit,

        KeyCode::Char('1') => {
            // Stay on env tabs; if somehow on Shell, jump to All
            if app.active_tab == Tab::Shell {
                app.active_tab = Tab::All;
                app.selected = 0;
                app.scroll_offset = 0;
            }
            EventOutcome::Continue
        }
        KeyCode::Char('2') => {
            app.active_tab = Tab::Shell;
            EventOutcome::Continue
        }

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
        KeyCode::PageUp => {
            for _ in 0..10 {
                app.move_up();
            }
            EventOutcome::Continue
        }
        KeyCode::PageDown => {
            for _ in 0..10 {
                app.move_down();
            }
            EventOutcome::Continue
        }

        KeyCode::Char('s') => {
            app.cycle_sort();
            EventOutcome::Continue
        }

        KeyCode::Char('/') => {
            app.searching = true;
            EventOutcome::Continue
        }

        KeyCode::Char(' ') => {
            app.toggle_expand();
            EventOutcome::Continue
        }

        KeyCode::Esc => {
            app.status_message = None;
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
            }
            EventOutcome::Continue
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

        _ => EventOutcome::Continue,
    }
}

fn handle_onboarding_key(key: KeyEvent, app: &mut AppState) -> EventOutcome {
    match key.code {
        KeyCode::Esc => {
            // Dismiss without saving — use whatever config was loaded
            app.onboarding = None;
        }
        KeyCode::Enter => {
            let confirmed = app.onboarding.as_mut().unwrap().advance();
            if confirmed {
                let result = app.onboarding.as_ref().unwrap().build_result();
                app.onboarding = None;
                app.onboarding_result = Some(result);
            }
        }
        KeyCode::Tab => {
            let ob = app.onboarding.as_mut().unwrap();
            if !ob.completions.is_empty() {
                ob.accept_completion();
            } else {
                let confirmed = ob.advance();
                if confirmed {
                    let result = app.onboarding.as_ref().unwrap().build_result();
                    app.onboarding = None;
                    app.onboarding_result = Some(result);
                }
            }
        }
        KeyCode::BackTab => {
            app.onboarding.as_mut().unwrap().retreat();
        }
        KeyCode::Up => {
            app.onboarding.as_mut().unwrap().completion_up();
        }
        KeyCode::Down => {
            app.onboarding.as_mut().unwrap().completion_down();
        }
        KeyCode::Backspace => {
            let ob = app.onboarding.as_mut().unwrap();
            ob.active_input_mut().pop();
            ob.refresh_completions();
        }
        KeyCode::Char(c) => {
            let ob = app.onboarding.as_mut().unwrap();
            ob.active_input_mut().push(c);
            ob.refresh_completions();
        }
        _ => {}
    }
    EventOutcome::Continue
}

fn handle_search_key(key: KeyEvent, app: &mut AppState) -> EventOutcome {
    match key.code {
        KeyCode::Esc => {
            app.search.clear();
            app.searching = false;
            app.selected = 0;
            app.scroll_offset = 0;
            EventOutcome::Continue
        }
        KeyCode::Backspace => {
            app.search.pop();
            app.selected = 0;
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
        KeyCode::Char(c) => {
            app.search.push(c);
            app.selected = 0;
            EventOutcome::Continue
        }
        _ => EventOutcome::Continue,
    }
}

fn handle_tab_manager_key(key: KeyEvent, app: &mut AppState) -> EventOutcome {
    match key.code {
        KeyCode::Char('q') => return EventOutcome::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return EventOutcome::Quit
        }
        KeyCode::Esc | KeyCode::Char('T') => {
            app.show_tab_manager = false;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.tab_manager_cursor = app.tab_manager_cursor.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.tab_manager_cursor + 1 < Tab::ALL.len() {
                app.tab_manager_cursor += 1;
            }
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            if let Some(tab) = Tab::ALL.get(app.tab_manager_cursor) {
                app.toggle_tab_visibility(*tab);
            }
        }
        _ => {}
    }
    EventOutcome::Continue
}

pub fn handle_mouse(mouse: MouseEvent, app: &mut AppState) -> EventOutcome {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let (col, row) = (mouse.column, mouse.row);

            // Tab manager button toggles the overlay
            if app.tab_manager_rect.contains(col, row) {
                app.show_tab_manager = !app.show_tab_manager;
                return EventOutcome::Continue;
            }

            // When overlay is open, clicks on items toggle tabs; clicks outside close it
            if app.show_tab_manager {
                for (i, rect) in app.tab_manager_item_rects.iter().enumerate() {
                    if rect.contains(col, row) {
                        app.tab_manager_cursor = i;
                        if let Some(tab) = Tab::ALL.get(i) {
                            app.toggle_tab_visibility(*tab);
                        }
                        return EventOutcome::Continue;
                    }
                }
                app.show_tab_manager = false;
                return EventOutcome::Continue;
            }

            // Shell tab item clicks — set cursor to clicked row
            if app.active_tab == Tab::Shell {
                for (i, rect) in app.shell.item_rects.iter().enumerate() {
                    if rect.contains(col, row) {
                        app.shell.cursor = i;
                        return EventOutcome::Continue;
                    }
                }
            }

            for (i, rect) in app.tab_rects.iter().enumerate() {
                if rect.contains(col, row) {
                    app.set_tab(i);
                    return EventOutcome::Continue;
                }
            }
            for (i, rect) in app.sort_rects.iter().enumerate() {
                if rect.contains(col, row) {
                    if let Some(field) = SortField::ALL.get(i) {
                        app.set_sort(*field);
                    }
                    return EventOutcome::Continue;
                }
            }
            EventOutcome::Continue
        }
        MouseEventKind::ScrollUp => {
            app.move_up();
            EventOutcome::Continue
        }
        MouseEventKind::ScrollDown => {
            app.move_down();
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
    fn slash_enters_search_mode() {
        let mut app = make_app(vec![]);
        assert!(!app.searching);
        handle_key(key(KeyCode::Char('/')), &mut app);
        assert!(app.searching);
    }

    #[test]
    fn search_chars_accumulate_in_search_mode() {
        let mut app = make_app(vec![]);
        handle_key(key(KeyCode::Char('/')), &mut app);
        handle_key(key(KeyCode::Char('a')), &mut app);
        handle_key(key(KeyCode::Char('p')), &mut app);
        handle_key(key(KeyCode::Char('i')), &mut app);
        assert_eq!(app.search, "api");
    }

    #[test]
    fn chars_do_not_enter_search_in_command_mode() {
        let mut app = make_app(vec![]);
        handle_key(key(KeyCode::Char('p')), &mut app);
        handle_key(key(KeyCode::Char('i')), &mut app);
        assert_eq!(app.search, "");
        assert!(!app.searching);
    }

    #[test]
    fn esc_clears_search_and_exits_search_mode() {
        let mut app = make_app(vec![]);
        app.search = "hello".to_string();
        app.searching = true;
        handle_key(key(KeyCode::Esc), &mut app);
        assert!(app.search.is_empty());
        assert!(!app.searching);
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

    #[test]
    fn space_in_command_mode_toggles_expand() {
        let mut app = make_app(vec![make_env("myenv")]);
        assert!(app.expanded_envs.is_empty());
        handle_key(key(KeyCode::Char(' ')), &mut app);
        assert_eq!(app.expanded_envs.len(), 1);
        handle_key(key(KeyCode::Char(' ')), &mut app);
        assert!(app.expanded_envs.is_empty());
    }

    #[test]
    fn space_in_search_mode_types_space() {
        let mut app = make_app(vec![make_env("myenv")]);
        app.searching = true;
        handle_key(key(KeyCode::Char('h')), &mut app);
        handle_key(key(KeyCode::Char(' ')), &mut app);
        handle_key(key(KeyCode::Char('w')), &mut app);
        assert_eq!(app.search, "h w");
    }
}
