use crate::env::{EnvKind, Environment, HealthStatus};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    All,
    Python,
    Node,
    Conda,
    Go,
    Ruby,
    Cargo,
    Java,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::All, Tab::Python, Tab::Node, Tab::Conda,
        Tab::Go, Tab::Ruby, Tab::Cargo, Tab::Java,
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
        }
    }

    pub fn index(&self) -> usize {
        Self::ALL.iter().position(|t| t == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortField {
    Size,
    Name,
    LastUsed,
    Health,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortField {
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
    pub envs: Vec<Environment>,
    pub active_tab: Tab,
    pub sort_field: SortField,
    pub sort_dir: SortDir,
    pub search: String,
    pub selected: usize,
    pub scroll_offset: usize,
    pub show_help: bool,
    pub confirm_delete: bool,
    pub pending_activation: Option<String>,
    pub status_message: Option<String>,
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
            envs,
            active_tab,
            sort_field,
            sort_dir: SortDir::Desc,
            search: String::new(),
            selected: 0,
            scroll_offset: 0,
            show_help: false,
            confirm_delete: false,
            pending_activation: None,
            status_message: None,
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
        self.clamp_scroll(20);
    }

    pub fn move_down(&mut self) {
        let count = self.filtered_envs().len();
        if self.selected + 1 < count {
            self.selected += 1;
        }
        self.clamp_scroll(20);
    }

    pub fn clamp_scroll(&mut self, visible_rows: usize) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + visible_rows {
            self.scroll_offset = self.selected + 1 - visible_rows;
        }
    }

    pub fn next_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()].clone();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn prev_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()].clone();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn cycle_sort(&mut self) {
        self.sort_field = self.sort_field.next();
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
        app.active_tab = Tab::Java; // last tab
        app.next_tab();
        assert_eq!(app.active_tab, Tab::All);
    }
}
