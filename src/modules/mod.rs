pub mod detect;
pub mod installer;
pub mod private_repo;
pub mod resolver;
pub mod zshrc;

use serde::Deserialize;

/// Agent context document embedded at compile time.
pub const AGENT_CONTEXT: &str = include_str!("AGENT_CONTEXT.md");

#[derive(Debug, Clone, Deserialize)]
pub struct Module {
    pub name: String,
    pub description: String,
    pub category: String,
    pub detect: DetectSpec,
    pub install: Option<InstallSpec>,
    pub zshrc: ZshrcSpec,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetectSpec {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallSpec {
    pub linux: Option<InstallOs>,
    pub macos: Option<InstallOs>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallOs {
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZshrcSpec {
    pub snippet: String,
    #[serde(default)]
    pub startup_ms_estimate: u64,
    #[serde(default = "default_order")]
    pub order: i32,
    pub user_extend: Option<String>,
}

fn default_order() -> i32 { 100 }

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleStatus {
    NotInstalled,
    InstalledUnmanaged,
    ManagedActive,
    ManagedInactive,
}

impl ModuleStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not installed",
            Self::InstalledUnmanaged => "unmanaged",
            Self::ManagedActive => "active",
            Self::ManagedInactive => "inactive",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModuleEntry {
    pub definition: Module,
    pub status: ModuleStatus,
    pub enabled: bool,
    /// Has an install command for the current platform.
    pub can_install: bool,
    /// Names of `depends_on` modules that are not currently installed.
    pub missing_deps: Vec<String>,
    /// Diff between canonical snippet and what's actually in .zshrc, if different.
    pub block_diff: Option<BlockDiff>,
    /// Whether the detail panel is expanded for this entry.
    pub expanded: bool,
}

// ── Unmanaged / custom block types ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UnmanagedBlock {
    pub index: usize,
    /// Raw content of the block (the .zshrc lines between clenv-managed blocks).
    pub content: String,
    pub line_count: usize,
    pub expanded: bool,
}

impl UnmanagedBlock {
    /// First non-blank line of content, truncated for display.
    pub fn label(&self) -> String {
        self.content.lines()
            .find(|l| !l.trim().is_empty())
            .map(|l| if l.len() > 50 { format!("{}…", &l[..50]) } else { l.to_string() })
            .unwrap_or_else(|| "(empty)".to_string())
    }
}

// ── Diff types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BlockDiff {
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Word-level spans for this line. Each span is (text, changed).
    /// `changed` = true means the word was added/removed (highlight it).
    pub spans: Vec<(String, bool)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLineKind {
    Equal,
    Removed,
    Added,
}

/// Load all built-in modules. Returns them sorted by zshrc.order.
/// Built-in TOMLs are embedded at compile time via include_str!().
pub fn load_builtin_modules() -> Vec<Module> {
    let raw: &[(&str, &str)] = &[
        ("homebrew-linux", include_str!("builtin/homebrew-linux.toml")),
        ("powerlevel10k", include_str!("builtin/powerlevel10k.toml")),
        ("zsh-autosuggestions", include_str!("builtin/zsh-autosuggestions.toml")),
        ("fast-syntax-highlighting", include_str!("builtin/fast-syntax-highlighting.toml")),
        ("fzf", include_str!("builtin/fzf.toml")),
        ("fzf-tab", include_str!("builtin/fzf-tab.toml")),
        ("mamba", include_str!("builtin/mamba.toml")),
        ("conda-aliases", include_str!("builtin/conda-aliases.toml")),
        ("uv", include_str!("builtin/uv.toml")),
        ("nvm", include_str!("builtin/nvm.toml")),
        ("bun", include_str!("builtin/bun.toml")),
        ("sdkman", include_str!("builtin/sdkman.toml")),
        ("zoxide", include_str!("builtin/zoxide.toml")),
        ("thefuck", include_str!("builtin/thefuck.toml")),
        ("jabba", include_str!("builtin/jabba.toml")),
    ];

    let mut modules: Vec<Module> = raw
        .iter()
        .filter_map(|(name, content)| {
            toml::from_str(content)
                .map_err(|e| eprintln!("Failed to parse module {name}: {e}"))
                .ok()
        })
        .collect();

    modules.sort_by_key(|m| m.zshrc.order);
    modules
}
