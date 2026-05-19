use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum OnboardingField {
    Roots,
    DepthLimit,
    Ignore,
}

pub struct OnboardingResult {
    pub roots: Vec<PathBuf>,
    pub depth_limit: usize,
    pub ignore: Vec<PathBuf>,
}

pub struct OnboardingState {
    pub field: OnboardingField,
    pub roots_input: String,
    pub depth_input: String,
    pub ignore_input: String,
    pub completions: Vec<String>,
    pub completion_idx: usize,
}

impl OnboardingState {
    pub fn new(roots: &[PathBuf], depth: usize) -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let roots_str = if roots.is_empty() {
            "~/".to_string()
        } else {
            roots
                .iter()
                .map(|p| abbreviate(&home, p))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut s = Self {
            field: OnboardingField::Roots,
            roots_input: roots_str,
            depth_input: depth.to_string(),
            ignore_input: String::new(),
            completions: Vec::new(),
            completion_idx: 0,
        };
        s.refresh_completions();
        s
    }

    pub fn active_input(&self) -> &str {
        match self.field {
            OnboardingField::Roots => &self.roots_input,
            OnboardingField::DepthLimit => &self.depth_input,
            OnboardingField::Ignore => &self.ignore_input,
        }
    }

    pub fn active_input_mut(&mut self) -> &mut String {
        match self.field {
            OnboardingField::Roots => &mut self.roots_input,
            OnboardingField::DepthLimit => &mut self.depth_input,
            OnboardingField::Ignore => &mut self.ignore_input,
        }
    }

    pub fn is_path_field(&self) -> bool {
        matches!(self.field, OnboardingField::Roots | OnboardingField::Ignore)
    }

    /// Advance to the next field. Returns `true` when already on the last field
    /// (caller should treat this as "confirmed").
    pub fn advance(&mut self) -> bool {
        match self.field {
            OnboardingField::Roots => {
                self.field = OnboardingField::DepthLimit;
                self.refresh_completions();
                false
            }
            OnboardingField::DepthLimit => {
                self.field = OnboardingField::Ignore;
                self.refresh_completions();
                false
            }
            OnboardingField::Ignore => true,
        }
    }

    pub fn retreat(&mut self) {
        self.field = match self.field {
            OnboardingField::Roots => OnboardingField::Roots,
            OnboardingField::DepthLimit => OnboardingField::Roots,
            OnboardingField::Ignore => OnboardingField::DepthLimit,
        };
        self.refresh_completions();
    }

    pub fn completion_up(&mut self) {
        if self.completion_idx > 0 {
            self.completion_idx -= 1;
        }
    }

    pub fn completion_down(&mut self) {
        if self.completion_idx + 1 < self.completions.len() {
            self.completion_idx += 1;
        }
    }

    /// Replace the last comma-separated path segment with the selected completion.
    pub fn accept_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let comp = self.completions[self.completion_idx].clone();
        let input = self.active_input_mut();
        let new_val = if let Some(comma) = input.rfind(',') {
            format!("{}, {}/", input[..comma].trim_end(), comp.trim_end_matches('/'))
        } else {
            format!("{}/", comp.trim_end_matches('/'))
        };
        *input = new_val;
        self.refresh_completions();
    }

    pub fn refresh_completions(&mut self) {
        if !self.is_path_field() {
            self.completions.clear();
            self.completion_idx = 0;
            return;
        }
        let last = self
            .active_input()
            .split(',')
            .last()
            .unwrap_or("")
            .trim()
            .to_string();
        self.completions = path_completions(&last);
        self.completion_idx = 0;
    }

    pub fn build_result(&self) -> OnboardingResult {
        let parse_paths = |s: &str| -> Vec<PathBuf> {
            s.split(',')
                .map(|p| expand_path(p.trim()))
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        };
        OnboardingResult {
            roots: parse_paths(&self.roots_input),
            depth_limit: self.depth_input.trim().parse().unwrap_or(10),
            ignore: parse_paths(&self.ignore_input),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn abbreviate(home: &PathBuf, p: &PathBuf) -> String {
    if let Ok(rel) = p.strip_prefix(home) {
        if rel.as_os_str().is_empty() {
            "~/".to_string()
        } else {
            format!("~/{}", rel.display())
        }
    } else {
        p.to_string_lossy().to_string()
    }
}

pub fn expand_path(s: &str) -> PathBuf {
    if s.is_empty() {
        return PathBuf::new();
    }
    if s == "~" || s == "~/" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return dirs::home_dir()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| PathBuf::from(s));
    }
    PathBuf::from(s)
}

fn expand_str(s: &str) -> String {
    if s == "~" || s == "~/" {
        return format!(
            "{}/",
            dirs::home_dir().unwrap_or_default().display()
        );
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), rest);
        }
    }
    s.to_string()
}

pub fn path_completions(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }
    let expanded = expand_str(input);
    let (dir, prefix): (PathBuf, String) = if expanded.ends_with('/') {
        (PathBuf::from(&expanded), String::new())
    } else {
        let p = std::path::Path::new(&expanded);
        let dir = p
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/"))
            .to_path_buf();
        let prefix = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        (dir, prefix)
    };

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let home = dirs::home_dir().unwrap_or_default();
    let mut results: Vec<String> = entries
        .filter_map(|e| {
            let e = e.ok()?;
            if !e.file_type().ok()?.is_dir() {
                return None;
            }
            let name = e.file_name().to_string_lossy().to_string();
            // Skip hidden dirs unless user typed a leading dot
            if name.starts_with('.') && !prefix.starts_with('.') {
                return None;
            }
            if !name.starts_with(&prefix) {
                return None;
            }
            let full = dir.join(&name);
            if let Ok(rel) = full.strip_prefix(&home) {
                Some(format!("~/{}", rel.display()))
            } else {
                Some(full.to_string_lossy().to_string())
            }
        })
        .take(6)
        .collect();
    results.sort();
    results
}
