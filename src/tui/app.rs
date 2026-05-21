use crate::env::{EnvKind, Environment, HealthStatus};
use super::onboarding::{OnboardingResult, OnboardingState};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Default)]
pub struct HitRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl HitRect {
    pub fn contains(&self, col: u16, row: u16) -> bool {
        col >= self.x
            && col < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    All,
    Python,
    Node,
    Conda,
    Go,
    Ruby,
    Cargo,
    Java,
    Shell,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::All, Tab::Python, Tab::Node, Tab::Conda,
        Tab::Go, Tab::Ruby, Tab::Cargo, Tab::Java, Tab::Shell,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Tab::All => "All",
            Tab::Python => "Python",
            Tab::Node => "Node",
            Tab::Conda => "Conda",
            Tab::Go => "Go",
            Tab::Ruby => "Ruby",
            Tab::Cargo => "Cargo",
            Tab::Java => "Java",
            Tab::Shell => "Shell",
        }
    }

    fn matches_kind(&self, kind: &EnvKind) -> bool {
        match self {
            Tab::All => true,
            Tab::Python => *kind == EnvKind::Python,
            Tab::Node => *kind == EnvKind::Node,
            Tab::Conda => *kind == EnvKind::Conda,
            Tab::Go => *kind == EnvKind::Go,
            Tab::Ruby => *kind == EnvKind::Ruby,
            Tab::Cargo => *kind == EnvKind::Cargo,
            Tab::Java => *kind == EnvKind::Java,
            Tab::Shell => false,
        }
    }

}

pub struct ShellTabState {
    pub entries: Vec<crate::modules::ModuleEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub item_rects: Vec<HitRect>,
    pub pending_enabled: HashMap<String, bool>,
    pub private_repo_last_sync: Option<std::time::SystemTime>,
}

impl Default for ShellTabState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            item_rects: Vec::new(),
            pending_enabled: HashMap::new(),
            private_repo_last_sync: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortField {
    Size,
    Name,
    LastUsed,
    Health,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortField {
    pub const ALL: &'static [SortField] = &[
        SortField::Size,
        SortField::Name,
        SortField::LastUsed,
        SortField::Health,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SortField::Size => "Size",
            SortField::Name => "Name",
            SortField::LastUsed => "Last Used",
            SortField::Health => "Health",
        }
    }

    pub fn next(&self) -> SortField {
        match self {
            SortField::Size => SortField::Name,
            SortField::Name => SortField::LastUsed,
            SortField::LastUsed => SortField::Health,
            SortField::Health => SortField::Size,
        }
    }
}

pub struct AppState {
    pub home_dir: PathBuf,
    pub envs: Vec<Environment>,
    pub active_tab: Tab,
    pub sort_field: SortField,
    pub sort_dir: SortDir,
    pub search: String,
    pub searching: bool,
    pub selected: usize,
    pub scroll_offset: usize,
    pub show_help: bool,
    pub confirm_delete: bool,
    pub status_message: Option<String>,
    pub expanded_envs: HashSet<PathBuf>,
    pub tab_rects: Vec<HitRect>,
    pub sort_rects: Vec<HitRect>,
    pub visible_rows: usize,
    pub hidden_tabs: HashSet<Tab>,
    pub show_tab_manager: bool,
    pub tab_manager_cursor: usize,
    pub tab_manager_rect: HitRect,
    pub tab_manager_item_rects: Vec<HitRect>,
    pub rescanning: bool,
    pub onboarding: Option<OnboardingState>,
    pub onboarding_result: Option<OnboardingResult>,
    pub shell: ShellTabState,
}

impl AppState {
    pub fn new(envs: Vec<Environment>, default_tab: &str, default_sort: &str) -> Self {
        let active_tab = match default_tab {
            "Python" => Tab::Python,
            "Node" => Tab::Node,
            "Conda" => Tab::Conda,
            "Go" => Tab::Go,
            "Ruby" => Tab::Ruby,
            "Cargo" => Tab::Cargo,
            "Java" => Tab::Java,
            _ => Tab::All,
        };
        let sort_field = match default_sort {
            "name" => SortField::Name,
            "last_used" => SortField::LastUsed,
            "health" => SortField::Health,
            _ => SortField::Size,
        };
        Self {
            home_dir: dirs::home_dir().unwrap_or_default(),
            envs,
            active_tab,
            sort_field,
            sort_dir: SortDir::Desc,
            search: String::new(),
            searching: false,
            selected: 0,
            scroll_offset: 0,
            show_help: false,
            confirm_delete: false,
            status_message: None,
            expanded_envs: HashSet::new(),
            tab_rects: Vec::new(),
            sort_rects: Vec::new(),
            visible_rows: 20,
            hidden_tabs: HashSet::new(),
            show_tab_manager: false,
            tab_manager_cursor: 0,
            tab_manager_rect: HitRect::default(),
            tab_manager_item_rects: Vec::new(),
            rescanning: false,
            onboarding: None,
            onboarding_result: None,
            shell: ShellTabState::default(),
        }
    }

    pub fn visible_tabs(&self) -> Vec<&Tab> {
        Tab::ALL
            .iter()
            .filter(|t| !self.hidden_tabs.contains(*t))
            .collect()
    }

    /// Toggle a tab's visibility. Refuses to hide the last visible tab.
    pub fn toggle_tab_visibility(&mut self, tab: Tab) {
        if self.hidden_tabs.contains(&tab) {
            self.hidden_tabs.remove(&tab);
        } else {
            let visible_count = Tab::ALL.len() - self.hidden_tabs.len();
            if visible_count <= 1 {
                return;
            }
            // If we're hiding the active tab, switch away first
            if self.active_tab == tab {
                if let Some(next) = Tab::ALL
                    .iter()
                    .find(|t| **t != tab && !self.hidden_tabs.contains(t))
                {
                    self.active_tab = *next;
                    self.selected = 0;
                    self.scroll_offset = 0;
                }
            }
            self.hidden_tabs.insert(tab);
        }
    }

    /// Returns envs filtered by active tab + search, sorted by sort_field/sort_dir.
    pub fn filtered_envs(&self) -> Vec<&Environment> {
        let query = self.search.to_lowercase();
        let mut envs: Vec<&Environment> = self
            .envs
            .iter()
            .filter(|e| self.active_tab.matches_kind(&e.kind))
            .filter(|e| {
                query.is_empty()
                    || e.name.to_lowercase().contains(&query)
                    || e.path.to_string_lossy().to_lowercase().contains(&query)
            })
            .collect();

        envs.sort_by(|a, b| {
            let ord = match self.sort_field {
                SortField::Size => a.size_bytes.cmp(&b.size_bytes),
                SortField::Name => a.name.cmp(&b.name),
                SortField::LastUsed => {
                    let ta = a.last_accessed.unwrap_or(SystemTime::UNIX_EPOCH);
                    let tb = b.last_accessed.unwrap_or(SystemTime::UNIX_EPOCH);
                    ta.cmp(&tb)
                }
                SortField::Health => {
                    let rank = |h: &HealthStatus| match h {
                        HealthStatus::Broken(_) => 0,
                        HealthStatus::Warnings(_) => 1,
                        HealthStatus::Ok => 2,
                        HealthStatus::Unknown => 3,
                    };
                    rank(&a.health).cmp(&rank(&b.health))
                }
            };
            if self.sort_dir == SortDir::Desc {
                ord.reverse()
            } else {
                ord
            }
        });

        envs
    }

    pub fn selected_env(&self) -> Option<&Environment> {
        self.filtered_envs().into_iter().nth(self.selected)
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.clamp_scroll();
    }

    pub fn move_down(&mut self) {
        let count = self.filtered_envs().len();
        if self.selected + 1 < count {
            self.selected += 1;
        }
        self.clamp_scroll();
    }

    pub fn clamp_scroll(&mut self) {
        let visible = self.visible_rows.max(1);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected + 1 - visible;
        }
    }

    pub fn next_tab(&mut self) {
        let visible = self.visible_tabs();
        if visible.is_empty() {
            return;
        }
        let pos = visible.iter().position(|t| **t == self.active_tab).unwrap_or(0);
        self.active_tab = *visible[(pos + 1) % visible.len()];
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn prev_tab(&mut self) {
        let visible = self.visible_tabs();
        if visible.is_empty() {
            return;
        }
        let pos = visible.iter().position(|t| **t == self.active_tab).unwrap_or(0);
        self.active_tab = *visible[(pos + visible.len() - 1) % visible.len()];
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn cycle_sort(&mut self) {
        let next = self.sort_field.next();
        // When the field wraps back to Size, toggle direction
        if next == SortField::Size {
            self.sort_dir = if self.sort_dir == SortDir::Asc {
                SortDir::Desc
            } else {
                SortDir::Asc
            };
        }
        self.sort_field = next;
    }

    /// Click a sort field: same field → toggle direction, different field → switch.
    pub fn set_sort(&mut self, field: SortField) {
        if self.sort_field == field {
            self.sort_dir = if self.sort_dir == SortDir::Asc {
                SortDir::Desc
            } else {
                SortDir::Asc
            };
        } else {
            self.sort_field = field;
        }
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// `idx` indexes into the currently visible tabs (matches tab_rects order).
    pub fn set_tab(&mut self, idx: usize) {
        let visible = self.visible_tabs();
        if let Some(tab) = visible.get(idx) {
            self.active_tab = **tab;
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    /// Replace env list after a background rescan, preserving selection and scroll.
    pub fn update_envs(&mut self, new_envs: Vec<Environment>) {
        let selected_path = self.selected_env().map(|e| e.path.clone());
        self.envs = new_envs;
        // Prune expanded set to paths that still exist
        let paths: HashSet<PathBuf> = self.envs.iter().map(|e| e.path.clone()).collect();
        self.expanded_envs.retain(|p| paths.contains(p));
        // Try to restore selection by path
        if let Some(path) = selected_path {
            let filtered = self.filtered_envs();
            if let Some(idx) = filtered.iter().position(|e| e.path == path) {
                self.selected = idx;
            } else {
                self.selected = self.selected.min(filtered.len().saturating_sub(1));
            }
        }
        self.clamp_scroll();
        let count = self.envs.len();
        self.status_message = Some(format!("Refreshed — {count} environments"));
        self.rescanning = false;
    }

    pub fn toggle_expand(&mut self) {
        if let Some(env) = self.selected_env() {
            let path = env.path.clone();
            if self.expanded_envs.contains(&path) {
                self.expanded_envs.remove(&path);
            } else {
                self.expanded_envs.insert(path);
            }
        }
    }

    pub fn load_shell_modules(&mut self, config: &crate::config::ModulesConfig) {
        let zshrc_path = config.zshrc_path.clone()
            .unwrap_or_else(|| self.home_dir.join(".zshrc"));

        let modules = crate::modules::load_builtin_modules();

        self.shell.entries = modules
            .into_iter()
            .map(|module| {
                let status = crate::modules::detect::module_status(&module, &zshrc_path);
                let enabled = config.enabled.contains(&module.name);
                crate::modules::ModuleEntry { definition: module, status, enabled }
            })
            .collect();

        // Initialize pending_enabled to mirror current enabled state
        for entry in &self.shell.entries {
            self.shell.pending_enabled.insert(
                entry.definition.name.clone(),
                entry.enabled,
            );
        }
    }

    pub fn selected_module(&self) -> Option<&crate::modules::ModuleEntry> {
        self.shell.entries.get(self.shell.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Environment;
    use std::path::PathBuf;

    fn make_env(kind: EnvKind, name: &str, size: u64) -> Environment {
        let mut e = Environment::new(kind, PathBuf::from(format!("/fake/{name}")));
        e.name = name.to_string();
        e.size_bytes = size;
        e.health = HealthStatus::Ok;
        e
    }

    #[test]
    fn filter_by_tab() {
        let envs = vec![
            make_env(EnvKind::Python, "venv1", 100),
            make_env(EnvKind::Node, "node1", 200),
        ];
        let mut app = AppState::new(envs, "All", "size");
        app.active_tab = Tab::Python;
        let filtered = app.filtered_envs();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "venv1");
    }

    #[test]
    fn sort_by_size_desc() {
        let envs = vec![
            make_env(EnvKind::Python, "small", 100),
            make_env(EnvKind::Python, "large", 1000),
        ];
        let app = AppState::new(envs, "All", "size");
        let filtered = app.filtered_envs();
        assert_eq!(filtered[0].name, "large");
    }

    #[test]
    fn search_filters_by_name() {
        let envs = vec![
            make_env(EnvKind::Python, "my-api", 100),
            make_env(EnvKind::Python, "other", 200),
        ];
        let mut app = AppState::new(envs, "All", "size");
        app.search = "api".to_string();
        let filtered = app.filtered_envs();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "my-api");
    }

    #[test]
    fn move_down_clamps_to_list_length() {
        let envs = vec![make_env(EnvKind::Python, "a", 1)];
        let mut app = AppState::new(envs, "All", "size");
        app.move_down();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn tab_cycling_wraps() {
        let mut app = AppState::new(vec![], "All", "size");
        app.active_tab = Tab::Shell; // last tab
        app.next_tab();
        assert_eq!(app.active_tab, Tab::All);
    }

    #[test]
    fn cycle_sort_toggles_direction_on_wrap() {
        let mut app = AppState::new(vec![], "All", "size");
        assert_eq!(app.sort_dir, SortDir::Desc);
        // cycle through all 4 fields back to Size
        app.cycle_sort(); // Name
        app.cycle_sort(); // LastUsed
        app.cycle_sort(); // Health
        app.cycle_sort(); // Size — wraps, toggles to Asc
        assert_eq!(app.sort_dir, SortDir::Asc);
        assert_eq!(app.sort_field, SortField::Size);
    }
}
